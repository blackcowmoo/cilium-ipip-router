use std::io;

use k8s_openapi::api::core::v1::Node;
use kube::client::Client;
use kube::ResourceExt;
use std::process::Command;

pub trait IpCommandExecutor {
    fn run(&self, args: &[&str]) -> io::Result<std::process::Output>;
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
            return Err(io::Error::other(format!(
                "ip command failed: {:?}",
                output.status
            )));
        }
        Ok(output)
    }
}

pub async fn get_local_node_ip() -> Option<String> {
    let hostname = std::env::var("HOSTNAME").ok()?;
    match Client::try_default().await {
        Ok(client) => {
            let nodes: kube::Api<Node> = kube::Api::all(client);

            match nodes.list(&Default::default()).await {
                Ok(node_list) => node_list
                    .into_iter()
                    .find(|n| n.metadata.name.as_deref() == Some(hostname.as_str()))
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

pub fn get_tunnel_name(node_name: &str) -> String {
    use md5::compute;
    let hash = compute(node_name);
    let hex_hash = format!("{:x}", hash);
    let truncated_hash = &hex_hash[0..11];
    format!("tun-{}", truncated_hash)
}

pub fn tunnel_exists<T: IpCommandExecutor>(executor: &T, tunnel_name: &str) -> io::Result<bool> {
    match executor.run(&["tunnel", "show", tunnel_name]) {
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
            let tunnel_name = get_tunnel_name(&node_name);

            match get_local_node_ip().await {
                Some(local_ip) => {
                    if !tunnel_exists(executor, &tunnel_name).unwrap_or(false) {
                        match executor.run(&[
                            "tunnel",
                            "add",
                            &tunnel_name,
                            "mode",
                            "ipip",
                            "local",
                            &local_ip,
                            "remote",
                            ip,
                        ]) {
                            Ok(output) => {
                                if !output.status.success() {
                                    log::error!(
                                        "Failed to create tunnel {}: command failed",
                                        tunnel_name
                                    );
                                } else {
                                    log::info!(
                                        "Created IPIP tunnel {} for node {}",
                                        tunnel_name,
                                        node_name
                                    );
                                }
                            }
                            Err(e) => {
                                log::error!("Failed to create tunnel {}: {}", tunnel_name, e);
                            }
                        }
                    } else {
                        log::info!(
                            "Tunnel {} for node {} already exists",
                            tunnel_name,
                            node_name
                        );
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
            match route_exists(executor, &cidr, &tunnel_name) {
                Ok(true) => {
                    log::info!(
                        "Route for node {} CIDR {} via tunnel {} already exists",
                        node_name,
                        cidr,
                        tunnel_name
                    );
                }
                Ok(false) => {
                    log::info!(
                        "Route for node {} CIDR {} via tunnel {} does not exist",
                        node_name,
                        cidr,
                        tunnel_name
                    );
                }
                Err(e) => {
                    log::warn!(
                        "Failed to check route for node {} CIDR {}: {}",
                        node_name,
                        cidr,
                        e
                    );
                }
            }

            if let Ok(output) = executor.run(&["route", "add", &cidr, "dev", &tunnel_name]) {
                if output.status.success() {
                    log::info!(
                        "Added route for node {} CIDR {} via tunnel {}",
                        node_name,
                        cidr,
                        tunnel_name
                    );
                } else {
                    log::error!(
                        "Failed to add route for node {} CIDR {}: command failed",
                        node_name,
                        cidr
                    );
                }
            } else {
                log::error!(
                    "Failed to add route for node {} CIDR {}: command error",
                    node_name,
                    cidr
                );
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
        if let Ok(output) = executor.run(&["route", "del", &cidr, "dev", &tunnel_name]) {
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
        } else {
            log::error!(
                "Failed to delete route for node {} CIDR {}: command error",
                node_name,
                cidr
            );
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
                Err(io::Error::new(io::ErrorKind::Other, "command failed"))
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

    struct ErrorMock;

    impl IpCommandExecutor for ErrorMock {
        fn run(&self, _args: &[&str]) -> io::Result<std::process::Output> {
            Err(io::Error::new(io::ErrorKind::Other, "command failed"))
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
