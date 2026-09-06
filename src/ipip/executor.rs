use std::io;

use k8s_openapi::api::core::v1::Node;
use kube::client::Client;
use kube::ResourceExt;
use std::process::Command;

pub trait IpCommandExecutor {
    fn run(&self, args: &[&str]) -> io::Result<std::process::Output>;
    fn disable_reverse_path_filter(&self, interface: &str) -> io::Result<()>;
}

pub struct IpCommand;

impl Default for IpCommand {
    fn default() -> Self {
        Self::new()
    }
}

impl IpCommand {
    pub fn new() -> Self {
        IpCommand
    }
}

impl IpCommandExecutor for IpCommand {
    fn run(&self, args: &[&str]) -> io::Result<std::process::Output> {
        if args.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "ip command requires at least one argument",
            ));
        }
        let output = Command::new("ip").args(args).output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(io::Error::other(format!(
                "ip command failed (exit: {:?}): {}",
                output.status, stderr
            )));
        }
        Ok(output)
    }

    fn disable_reverse_path_filter(&self, interface: &str) -> io::Result<()> {
        std::fs::write(
            format!("/proc/sys/net/ipv4/conf/{interface}/rp_filter"),
            b"0\n",
        )
    }
}

pub async fn get_local_node_ip() -> Option<String> {
    let node_name = get_local_node_name()?;
    match Client::try_default().await {
        Ok(client) => {
            let nodes: kube::Api<Node> = kube::Api::all(client);

            match nodes.list(&Default::default()).await {
                Ok(node_list) => node_list
                    .into_iter()
                    .find(|n| n.metadata.name.as_deref() == Some(node_name.as_str()))
                    .and_then(|node| get_node_ip(&node)),
                Err(e) => {
                    log::warn!("Failed to list nodes: {}", e);
                    None
                }
            }
        }
        Err(e) => {
            log::warn!("Failed to create Kubernetes client: {}", e);
            None
        }
    }
}

pub fn get_node_ip(node: &Node) -> Option<String> {
    node.status
        .as_ref()?
        .addresses
        .as_ref()?
        .iter()
        .find(|addr| addr.type_ == "ExternalIP" || addr.type_ == "InternalIP")
        .map(|addr| addr.address.clone())
}

pub fn get_node_cidr(node: &Node) -> Option<String> {
    node.spec.as_ref()?.pod_cidr.clone()
}

fn local_node_name_from(node_name: Option<String>, hostname: Option<String>) -> Option<String> {
    node_name.filter(|name| !name.is_empty()).or(hostname)
}

pub fn get_local_node_name() -> Option<String> {
    local_node_name_from(
        std::env::var("NODE_NAME").ok(),
        std::env::var("HOSTNAME").ok(),
    )
}

pub fn get_tunnel_name(node_name: &str) -> String {
    use md5::compute;
    let hash = compute(node_name);
    let hex_hash = format!("{:x}", hash);
    let truncated_hash = &hex_hash[0..11];
    format!("tun-{}", truncated_hash)
}

pub fn tunnel_exists<T: IpCommandExecutor>(executor: &T, tunnel_name: &str) -> io::Result<bool> {
    // `ip tunnel show <missing-name>` may still succeed and print the fallback
    // tunl0 device. Querying the link by exact device name fails when absent.
    match executor.run(&["link", "show", "dev", tunnel_name]) {
        Ok(output) => Ok(output.status.success()),
        Err(_) => Ok(false),
    }
}

pub fn route_exists<T: IpCommandExecutor>(
    executor: &T,
    cidr: &str,
    tunnel_name: &str,
) -> io::Result<bool> {
    match executor.run(&["route", "show", "to", cidr]) {
        Ok(output) => {
            if output.status.success() {
                let output_str = String::from_utf8_lossy(&output.stdout);
                Ok(output_str.contains(tunnel_name))
            } else {
                Ok(false)
            }
        }
        Err(_) => Ok(false),
    }
}

pub fn ensure_tunnel<T: IpCommandExecutor>(
    executor: &T,
    tunnel_name: &str,
    local_ip: &str,
    remote_ip: &str,
) -> io::Result<()> {
    if !tunnel_exists(executor, tunnel_name)? {
        executor.run(&[
            "tunnel",
            "add",
            tunnel_name,
            "mode",
            "ipip",
            "local",
            local_ip,
            "remote",
            remote_ip,
        ])?;
        log::info!("Created IPIP tunnel {}", tunnel_name);
    }

    // A newly-created tunnel is DOWN and Linux refuses routes through it until
    // it is brought up. Running this for an existing tunnel is idempotent.
    executor.run(&["link", "set", "dev", tunnel_name, "up"])?;

    // IPIP traffic is asymmetric from the tunnel interface's perspective.
    // Strict reverse-path filtering inherited from the node drops decapsulated
    // packets before they can reach the destination pod.
    executor.disable_reverse_path_filter(tunnel_name)?;
    Ok(())
}

pub async fn update_route_with_executor<T: IpCommandExecutor>(node: Node, executor: &T) {
    let node_name = node.name_any();
    let node_ip = get_node_ip(&node);
    let node_cidr = get_node_cidr(&node);

    if node_cidr.is_none() {
        log::info!(
            "Node {} does not have a CIDR, skipping tunnel creation",
            node_name
        );
        return;
    }

    match node_ip {
        Some(ref ip) => {
            if get_local_node_name().as_deref() == Some(node_name.as_str()) {
                log::info!("Skipping route creation for local node {}", node_name);
                return;
            }
            let tunnel_name = get_tunnel_name(&node_name);

            match get_local_node_ip().await {
                Some(local_ip) => {
                    if let Err(e) = ensure_tunnel(executor, &tunnel_name, &local_ip, ip) {
                        log::error!("Failed to configure tunnel {}: {}", tunnel_name, e);
                        return;
                    }
                }
                None => {
                    log::warn!(
                        "Could not determine local node IP, skipping tunnel creation for {}",
                        node_name
                    );
                }
            }

            let cidr = node_cidr.unwrap();
            let route_is_configured = match route_exists(executor, &cidr, &tunnel_name) {
                Ok(true) => {
                    log::info!(
                        "Route for node {} CIDR {} via tunnel {} already exists",
                        node_name,
                        cidr,
                        tunnel_name
                    );
                    true
                }
                Ok(false) => {
                    log::info!(
                        "Route for node {} CIDR {} via tunnel {} does not exist",
                        node_name,
                        cidr,
                        tunnel_name
                    );
                    false
                }
                Err(e) => {
                    log::warn!(
                        "Failed to check route for node {} CIDR {}: {}",
                        node_name,
                        cidr,
                        e
                    );
                    false
                }
            };

            if route_is_configured {
                return;
            }

            match executor.run(&["route", "add", &cidr, "dev", &tunnel_name]) {
                Ok(output) => {
                    if output.status.success() {
                        log::info!(
                            "Added route for node {} CIDR {} via tunnel {}",
                            node_name,
                            cidr,
                            tunnel_name
                        );
                    } else {
                        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                        log::error!(
                            "Failed to add route for node {} CIDR {}: ip route add failed: {}",
                            node_name,
                            cidr,
                            stderr
                        );
                    }
                }
                Err(e) => {
                    log::error!(
                        "Failed to add route for node {} CIDR {}: {}",
                        node_name,
                        cidr,
                        e
                    );
                }
            }
        }
        None => {
            log::warn!("No IP address found for node {}", node_name);
        }
    }
}

pub async fn delete_route_with_executor<T: IpCommandExecutor>(node: Node, executor: &T) {
    let node_name = node.name_any();
    let node_ip = get_node_ip(&node);
    let node_cidr = get_node_cidr(&node);
    let tunnel_name = get_tunnel_name(&node_name);

    if let (Some(cidr), Some(_ip)) = (node_cidr, node_ip) {
        match executor.run(&["route", "del", &cidr, "dev", &tunnel_name]) {
            Ok(output) => {
                if output.status.success() {
                    log::info!(
                        "Deleted route for node {} CIDR {} via tunnel {}",
                        node_name,
                        cidr,
                        tunnel_name
                    );
                } else {
                    log::error!(
                        "Failed to delete route for node {} CIDR {}: command failed",
                        node_name,
                        cidr
                    );
                }
            }
            Err(e) => {
                log::error!(
                    "Failed to delete route for node {} CIDR {}: {}",
                    node_name,
                    cidr,
                    e
                );
            }
        }
    }

    if let Ok(output) = executor.run(&["tunnel", "del", &tunnel_name]) {
        if output.status.success() {
            log::info!("Deleted IPIP tunnel {} for node {}", tunnel_name, node_name);
        } else {
            log::error!("Failed to delete tunnel {}: command failed", tunnel_name);
        }
    } else {
        log::error!("Failed to delete tunnel {}: command error", tunnel_name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::{NodeAddress, NodeSpec, NodeStatus};
    use std::io::ErrorKind;
    use std::sync::Mutex;

    #[test]
    fn test_get_node_ip_with_external_ip() {
        let node = Node {
            metadata: Default::default(),
            spec: Default::default(),
            status: Some(NodeStatus {
                addresses: Some(vec![NodeAddress {
                    type_: "ExternalIP".to_string(),
                    address: "192.168.1.1".to_string(),
                }]),
                ..Default::default()
            }),
        };
        let ip = get_node_ip(&node);
        assert_eq!(ip, Some("192.168.1.1".to_string()));
    }

    #[test]
    fn test_get_node_ip_with_internal_ip() {
        let node = Node {
            metadata: Default::default(),
            spec: Default::default(),
            status: Some(NodeStatus {
                addresses: Some(vec![NodeAddress {
                    type_: "InternalIP".to_string(),
                    address: "10.0.0.1".to_string(),
                }]),
                ..Default::default()
            }),
        };
        let ip = get_node_ip(&node);
        assert_eq!(ip, Some("10.0.0.1".to_string()));
    }

    #[test]
    fn test_get_node_ip_with_no_addresses() {
        let node = Node {
            metadata: Default::default(),
            spec: Default::default(),
            status: Some(NodeStatus {
                addresses: Some(vec![]),
                ..Default::default()
            }),
        };
        let ip = get_node_ip(&node);
        assert_eq!(ip, None);
    }

    #[test]
    fn test_get_node_ip_with_no_status() {
        let node = Node {
            metadata: Default::default(),
            spec: Default::default(),
            status: None,
        };
        let ip = get_node_ip(&node);
        assert_eq!(ip, None);
    }

    #[test]
    fn test_get_node_cidr_with_cidr() {
        let node = Node {
            metadata: Default::default(),
            spec: Some(NodeSpec {
                pod_cidr: Some("10.244.0.0/24".to_string()),
                ..Default::default()
            }),
            status: None,
        };
        let cidr = get_node_cidr(&node);
        assert_eq!(cidr, Some("10.244.0.0/24".to_string()));
    }

    #[test]
    fn test_get_node_cidr_with_no_cidr() {
        let node = Node {
            metadata: Default::default(),
            spec: Some(NodeSpec {
                pod_cidr: None,
                ..Default::default()
            }),
            status: None,
        };
        let cidr = get_node_cidr(&node);
        assert_eq!(cidr, None);
    }

    #[test]
    fn test_get_node_cidr_with_no_spec() {
        let node = Node {
            metadata: Default::default(),
            spec: None,
            status: None,
        };
        let cidr = get_node_cidr(&node);
        assert_eq!(cidr, None);
    }

    struct MockExecutor {
        should_succeed: bool,
    }

    impl IpCommandExecutor for MockExecutor {
        fn run(&self, _args: &[&str]) -> io::Result<std::process::Output> {
            if self.should_succeed {
                Ok(std::process::Output {
                    status: std::process::ExitStatus::default(),
                    stdout: vec![],
                    stderr: vec![],
                })
            } else {
                Err(io::Error::other("command failed"))
            }
        }

        fn disable_reverse_path_filter(&self, _interface: &str) -> io::Result<()> {
            if self.should_succeed {
                Ok(())
            } else {
                Err(io::Error::other("command failed"))
            }
        }
    }

    #[test]
    fn test_tunnel_exists_success() {
        let mock = MockExecutor {
            should_succeed: true,
        };
        let result = tunnel_exists(&mock, "tun-test");
        assert!(result.unwrap());
    }

    #[test]
    fn test_tunnel_exists_failure() {
        let mock = MockExecutor {
            should_succeed: false,
        };
        let result = tunnel_exists(&mock, "tun-nonexistent");
        assert!(!result.unwrap());
    }

    struct RouteMock {
        has_tunnel: bool,
    }

    impl IpCommandExecutor for RouteMock {
        fn run(&self, _args: &[&str]) -> io::Result<std::process::Output> {
            let stdout = if self.has_tunnel {
                b"10.244.0.0/24 via tun-test dev tun-test\n".to_vec()
            } else {
                b"10.244.1.0/24 via tun-other dev tun-other\n".to_vec()
            };
            Ok(std::process::Output {
                status: std::process::ExitStatus::default(),
                stdout,
                stderr: vec![],
            })
        }

        fn disable_reverse_path_filter(&self, _interface: &str) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn test_route_exists_success_with_tunnel() {
        let mock = RouteMock { has_tunnel: true };
        let result = route_exists(&mock, "10.244.0.0/24", "tun-test");
        assert!(result.unwrap());
    }

    #[test]
    fn test_route_exists_success_without_tunnel() {
        let mock = RouteMock { has_tunnel: false };
        let result = route_exists(&mock, "10.244.0.0/24", "tun-test");
        assert!(!result.unwrap());
    }

    struct TunnelMock {
        tunnel_exists: bool,
        calls: Mutex<Vec<Vec<String>>>,
    }

    impl IpCommandExecutor for TunnelMock {
        fn run(&self, args: &[&str]) -> io::Result<std::process::Output> {
            self.calls
                .lock()
                .unwrap()
                .push(args.iter().map(|arg| (*arg).to_string()).collect());

            if args.starts_with(&["link", "show", "dev"]) && !self.tunnel_exists {
                return Err(io::Error::new(io::ErrorKind::NotFound, "not found"));
            }

            Ok(std::process::Output {
                status: std::process::ExitStatus::default(),
                stdout: vec![],
                stderr: vec![],
            })
        }

        fn disable_reverse_path_filter(&self, interface: &str) -> io::Result<()> {
            self.calls.lock().unwrap().push(vec![
                "rp_filter".to_string(),
                interface.to_string(),
                "0".to_string(),
            ]);
            Ok(())
        }
    }

    #[test]
    fn test_ensure_tunnel_creates_and_brings_interface_up() {
        let mock = TunnelMock {
            tunnel_exists: false,
            calls: Mutex::new(vec![]),
        };

        ensure_tunnel(&mock, "tun-test", "192.0.2.1", "192.0.2.2").unwrap();

        assert_eq!(
            *mock.calls.lock().unwrap(),
            vec![
                vec!["link", "show", "dev", "tun-test"],
                vec![
                    "tunnel",
                    "add",
                    "tun-test",
                    "mode",
                    "ipip",
                    "local",
                    "192.0.2.1",
                    "remote",
                    "192.0.2.2",
                ],
                vec!["link", "set", "dev", "tun-test", "up"],
                vec!["rp_filter", "tun-test", "0"],
            ]
        );
    }

    #[test]
    fn test_ensure_tunnel_brings_existing_interface_up() {
        let mock = TunnelMock {
            tunnel_exists: true,
            calls: Mutex::new(vec![]),
        };

        ensure_tunnel(&mock, "tun-test", "192.0.2.1", "192.0.2.2").unwrap();

        assert_eq!(
            *mock.calls.lock().unwrap(),
            vec![
                vec!["link", "show", "dev", "tun-test"],
                vec!["link", "set", "dev", "tun-test", "up"],
                vec!["rp_filter", "tun-test", "0"],
            ]
        );
    }

    struct ErrorMock;

    impl IpCommandExecutor for ErrorMock {
        fn run(&self, _args: &[&str]) -> io::Result<std::process::Output> {
            Err(io::Error::other("command failed"))
        }

        fn disable_reverse_path_filter(&self, _interface: &str) -> io::Result<()> {
            Err(io::Error::other("command failed"))
        }
    }

    #[test]
    fn test_tunnel_exists_error() {
        let mock = ErrorMock;
        let result = tunnel_exists(&mock, "tun-test");
        assert!(!result.unwrap());
    }

    #[test]
    fn test_route_exists_error() {
        let mock = ErrorMock;
        let result = route_exists(&mock, "10.244.0.0/24", "tun-test");
        assert!(!result.unwrap());
    }

    #[test]
    fn test_ip_command_executor_run_empty_args() {
        let cmd = IpCommand::new();
        let result = cmd.run(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_node_name_prefers_downward_api_value() {
        assert_eq!(
            local_node_name_from(Some("node-a".into()), Some("pod-name".into())),
            Some("node-a".into())
        );
    }

    #[test]
    fn test_node_name_falls_back_to_hostname() {
        assert_eq!(
            local_node_name_from(None, Some("node-a".into())),
            Some("node-a".into())
        );
    }

    #[test]
    fn test_ip_command_executor_run() {
        let cmd = IpCommand::new();
        let result = cmd.run(&["link", "show", "lo"]);
        match result {
            Ok(_) => {}
            Err(e) if e.kind() == ErrorKind::NotFound => {
                eprintln!("Skipping test: ip command not found in test environment");
            }
            Err(e) => panic!("Unexpected error: {}", e),
        }
    }
}
