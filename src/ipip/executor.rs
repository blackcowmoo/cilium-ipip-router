use std::io;
use std::net::Ipv4Addr;

use k8s_openapi::api::core::v1::Node;
use kube::client::Client;
use kube::ResourceExt;
use std::process::Command;

pub const NODE_GROUP_LABEL_ENV: &str = "NODE_GROUP_LABEL";

/// Controls how remote nodes are grouped for routing decisions.
///
/// When `node_group_label` is set, equal label values and a pair of missing
/// labels identify the same group. Direct routing also requires an on-link
/// remote InternalIP; all other cases use IPIP.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RouteConfig {
    node_group_label: Option<String>,
}

impl RouteConfig {
    pub fn from_env() -> Self {
        Self::new(std::env::var(NODE_GROUP_LABEL_ENV).ok())
    }

    pub fn new(node_group_label: Option<String>) -> Self {
        Self {
            node_group_label: node_group_label
                .map(|label| label.trim().to_string())
                .filter(|label| !label.is_empty()),
        }
    }

    pub fn node_group_label(&self) -> Option<&str> {
        self.node_group_label.as_deref()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouteMode {
    Direct,
    Ipip,
}

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

async fn get_local_node() -> Option<Node> {
    let node_name = get_local_node_name()?;
    match Client::try_default().await {
        Ok(client) => {
            let nodes: kube::Api<Node> = kube::Api::all(client);

            match nodes.list(&Default::default()).await {
                Ok(node_list) => node_list
                    .into_iter()
                    .find(|n| n.metadata.name.as_deref() == Some(node_name.as_str())),
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

pub async fn get_local_node_ip() -> Option<String> {
    get_local_node().await.and_then(|node| get_node_ip(&node))
}

pub fn get_node_ip(node: &Node) -> Option<String> {
    let addresses = node.status.as_ref()?.addresses.as_ref()?;
    get_node_internal_ip(node).or_else(|| {
        addresses
            .iter()
            .find(|addr| addr.type_ == "ExternalIP")
            .map(|address| address.address.clone())
    })
}

fn get_node_internal_ip(node: &Node) -> Option<String> {
    node.status
        .as_ref()?
        .addresses
        .as_ref()?
        .iter()
        .find(|addr| addr.type_ == "InternalIP")
        .map(|address| address.address.clone())
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

fn get_local_ipv4_prefix(address_output: &[u8], local_ip: &str) -> Option<u8> {
    let local_ip = local_ip.parse::<Ipv4Addr>().ok()?;
    let output = String::from_utf8_lossy(address_output);

    output.lines().find_map(|line| {
        let fields = line.split_ascii_whitespace().collect::<Vec<_>>();
        fields.windows(2).find_map(|pair| {
            if pair[0] != "inet" {
                return None;
            }
            let (address, prefix) = pair[1].split_once('/')?;
            let address = address.parse::<Ipv4Addr>().ok()?;
            let prefix = prefix.parse::<u8>().ok()?;
            (address == local_ip && prefix <= 32).then_some(prefix)
        })
    })
}

fn is_same_ipv4_subnet(local_ip: &str, remote_ip: &str, prefix: u8) -> bool {
    let (Ok(local_ip), Ok(remote_ip)) =
        (local_ip.parse::<Ipv4Addr>(), remote_ip.parse::<Ipv4Addr>())
    else {
        return false;
    };
    if prefix > 32 {
        return false;
    }

    let mask = if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix)
    };
    u32::from(local_ip) & mask == u32::from(remote_ip) & mask
}

pub fn get_route_mode<T: IpCommandExecutor>(
    local_node: &Node,
    remote_node: &Node,
    config: &RouteConfig,
    executor: &T,
) -> RouteMode {
    let Some(label_key) = config.node_group_label() else {
        return RouteMode::Ipip;
    };

    let local_group = local_node
        .metadata
        .labels
        .as_ref()
        .and_then(|labels| labels.get(label_key))
        .filter(|value| !value.is_empty());
    let remote_group = remote_node
        .metadata
        .labels
        .as_ref()
        .and_then(|labels| labels.get(label_key))
        .filter(|value| !value.is_empty());

    let same_group = match (local_group, remote_group) {
        (Some(local), Some(remote)) => local == remote,
        (None, None) => true,
        _ => false,
    };
    if !same_group {
        return RouteMode::Ipip;
    }

    let Some(local_ip) = get_node_internal_ip(local_node) else {
        return RouteMode::Ipip;
    };
    let Some(remote_ip) = get_node_internal_ip(remote_node) else {
        return RouteMode::Ipip;
    };

    match executor.run(&["-o", "-4", "addr", "show"]) {
        Ok(output) if output.status.success() => {
            let Some(prefix) = get_local_ipv4_prefix(&output.stdout, &local_ip) else {
                log::warn!(
                    "Could not find subnet prefix for local InternalIP {}. Using IPIP",
                    local_ip
                );
                return RouteMode::Ipip;
            };
            if is_same_ipv4_subnet(&local_ip, &remote_ip, prefix) {
                RouteMode::Direct
            } else {
                RouteMode::Ipip
            }
        }
        Ok(_) => RouteMode::Ipip,
        Err(error) => {
            log::warn!(
                "Failed to read the local InternalIP subnet: {}. Using IPIP",
                error
            );
            RouteMode::Ipip
        }
    }
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

fn delete_tunnel_if_exists<T: IpCommandExecutor>(
    executor: &T,
    tunnel_name: &str,
) -> io::Result<()> {
    if tunnel_exists(executor, tunnel_name)? {
        executor.run(&["tunnel", "del", tunnel_name])?;
    }
    Ok(())
}

/// Reconciles the route to a remote node's PodCIDR.
///
/// Nodes in the same configured group use a direct next-hop route when the
/// remote InternalIP is on-link. All other cases use IPIP.
pub fn reconcile_route_with_executor<T: IpCommandExecutor>(
    local_node: &Node,
    remote_node: &Node,
    config: &RouteConfig,
    executor: &T,
) -> io::Result<()> {
    let local_name = local_node.name_any();
    let remote_name = remote_node.name_any();
    if local_name == remote_name {
        log::info!("Skipping route creation for local node {}", local_name);
        return Ok(());
    }

    let Some(cidr) = get_node_cidr(remote_node) else {
        log::info!(
            "Node {} does not have a CIDR, skipping route creation",
            remote_name
        );
        return Ok(());
    };
    let Some(remote_ip) = get_node_ip(remote_node) else {
        log::warn!("No IP address found for node {}", remote_name);
        return Ok(());
    };

    let tunnel_name = get_tunnel_name(&remote_name);
    match get_route_mode(local_node, remote_node, config, executor) {
        RouteMode::Direct => {
            executor.run(&["route", "replace", &cidr, "via", &remote_ip])?;
            delete_tunnel_if_exists(executor, &tunnel_name)?;
            log::info!(
                "Configured direct route for node {} CIDR {} via {}",
                remote_name,
                cidr,
                remote_ip
            );
        }
        RouteMode::Ipip => {
            let Some(local_ip) = get_node_ip(local_node) else {
                log::warn!(
                    "No IP address found for local node {}, skipping route for {}",
                    local_name,
                    remote_name
                );
                return Ok(());
            };
            ensure_tunnel(executor, &tunnel_name, &local_ip, &remote_ip)?;
            executor.run(&["route", "replace", &cidr, "dev", &tunnel_name])?;
            log::info!(
                "Configured IPIP route for node {} CIDR {} via tunnel {}",
                remote_name,
                cidr,
                tunnel_name
            );
        }
    }

    Ok(())
}

pub async fn update_route_with_executor<T: IpCommandExecutor>(node: Node, executor: &T) {
    let node_name = node.name_any();
    let Some(local_node) = get_local_node().await else {
        log::warn!(
            "Could not determine local node, skipping route creation for {}",
            node_name
        );
        return;
    };

    if let Err(error) =
        reconcile_route_with_executor(&local_node, &node, &RouteConfig::from_env(), executor)
    {
        log::error!(
            "Failed to reconcile route for node {}: {}",
            node_name,
            error
        );
    }
}

pub async fn delete_route_with_executor<T: IpCommandExecutor>(node: Node, executor: &T) {
    let node_name = node.name_any();
    let node_cidr = get_node_cidr(&node);
    let tunnel_name = get_tunnel_name(&node_name);

    if let Some(cidr) = node_cidr {
        match executor.run(&["route", "del", &cidr]) {
            Ok(output) => {
                if output.status.success() {
                    log::info!("Deleted route for node {} CIDR {}", node_name, cidr);
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

    match delete_tunnel_if_exists(executor, &tunnel_name) {
        Ok(()) => log::info!(
            "Cleaned up IPIP tunnel {} for node {}",
            tunnel_name,
            node_name
        ),
        Err(error) => log::error!(
            "Failed to delete tunnel {} for node {}: {}",
            tunnel_name,
            node_name,
            error
        ),
    }
}

#[cfg(test)]
#[path = "routing_tests.rs"]
mod routing_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::{NodeAddress, NodeSpec, NodeStatus};
    use std::collections::BTreeMap;
    use std::io::ErrorKind;
    use std::sync::Mutex;

    fn routing_node(name: &str, ip: &str, cidr: Option<&str>, group: Option<&str>) -> Node {
        let mut node = Node::default();
        node.metadata.name = Some(name.to_string());
        node.metadata.labels = group.map(|value| {
            BTreeMap::from([("router.example.com/group".to_string(), value.to_string())])
        });
        node.spec = Some(NodeSpec {
            pod_cidr: cidr.map(str::to_string),
            ..Default::default()
        });
        node.status = Some(NodeStatus {
            addresses: Some(vec![NodeAddress {
                type_: "InternalIP".to_string(),
                address: ip.to_string(),
            }]),
            ..Default::default()
        });
        node
    }

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
    fn test_get_node_ip_prefers_internal_ip() {
        let node = Node {
            metadata: Default::default(),
            spec: Default::default(),
            status: Some(NodeStatus {
                addresses: Some(vec![
                    NodeAddress {
                        type_: "ExternalIP".to_string(),
                        address: "192.0.2.10".to_string(),
                    },
                    NodeAddress {
                        type_: "InternalIP".to_string(),
                        address: "10.0.0.10".to_string(),
                    },
                ]),
                ..Default::default()
            }),
        };

        assert_eq!(get_node_ip(&node), Some("10.0.0.10".to_string()));
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

    #[test]
    fn test_route_config_trims_empty_values() {
        assert_eq!(RouteConfig::new(Some("  ".into())).node_group_label(), None);
        assert_eq!(
            RouteConfig::new(Some("  router.example.com/group  ".into())).node_group_label(),
            Some("router.example.com/group")
        );
    }

    struct SubnetLookupMock {
        addresses: Option<&'static str>,
    }

    impl IpCommandExecutor for SubnetLookupMock {
        fn run(&self, args: &[&str]) -> io::Result<std::process::Output> {
            assert_eq!(args, ["-o", "-4", "addr", "show"]);
            let Some(addresses) = self.addresses else {
                return Err(io::Error::other("address lookup failed"));
            };
            Ok(std::process::Output {
                status: std::process::ExitStatus::default(),
                stdout: addresses.as_bytes().to_vec(),
                stderr: vec![],
            })
        }

        fn disable_reverse_path_filter(&self, _interface: &str) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn test_same_non_empty_node_group_uses_direct_route() {
        let local = routing_node("node-a", "192.0.2.1", Some("10.0.1.0/24"), Some("rack-a"));
        let remote = routing_node("node-b", "192.0.2.2", Some("10.0.2.0/24"), Some("rack-a"));
        let config = RouteConfig::new(Some("router.example.com/group".into()));
        let mock = SubnetLookupMock {
            addresses: Some("2: eth0    inet 192.0.2.1/24 brd 192.0.2.255 scope global eth0"),
        };

        assert_eq!(
            get_route_mode(&local, &remote, &config, &mock),
            RouteMode::Direct
        );
    }

    #[test]
    fn test_nodes_without_group_labels_share_default_group() {
        let local = routing_node("node-a", "192.0.2.1", Some("10.0.1.0/24"), None);
        let remote = routing_node("node-b", "192.0.2.2", Some("10.0.2.0/24"), None);
        let config = RouteConfig::new(Some("router.example.com/group".into()));
        let mock = SubnetLookupMock {
            addresses: Some("2: eth0    inet 192.0.2.1/24 brd 192.0.2.255 scope global eth0"),
        };

        assert_eq!(
            get_route_mode(&local, &remote, &config, &mock),
            RouteMode::Direct
        );
    }

    #[test]
    fn test_different_or_one_missing_node_group_uses_ipip() {
        let local = routing_node("node-a", "192.0.2.1", Some("10.0.1.0/24"), Some("rack-a"));
        let different = routing_node("node-b", "192.0.2.2", Some("10.0.2.0/24"), Some("rack-b"));
        let missing = routing_node("node-c", "192.0.2.3", Some("10.0.3.0/24"), None);
        let config = RouteConfig::new(Some("router.example.com/group".into()));
        let mock = SubnetLookupMock { addresses: None };

        assert_eq!(
            get_route_mode(&local, &different, &config, &mock),
            RouteMode::Ipip
        );
        assert_eq!(
            get_route_mode(&local, &missing, &config, &mock),
            RouteMode::Ipip
        );
        assert_eq!(
            get_route_mode(&local, &different, &RouteConfig::default(), &mock),
            RouteMode::Ipip
        );
    }

    #[test]
    fn test_same_group_on_different_subnet_uses_ipip() {
        let local = routing_node("node-a", "192.0.2.1", Some("10.0.1.0/24"), Some("rack-a"));
        let remote = routing_node(
            "node-b",
            "198.51.100.2",
            Some("10.0.2.0/24"),
            Some("rack-a"),
        );
        let config = RouteConfig::new(Some("router.example.com/group".into()));
        let mock = SubnetLookupMock {
            addresses: Some("2: eth0    inet 192.0.2.1/24 brd 192.0.2.255 scope global eth0"),
        };

        assert_eq!(
            get_route_mode(&local, &remote, &config, &mock),
            RouteMode::Ipip
        );
    }

    #[test]
    fn test_same_group_subnet_lookup_failure_uses_ipip() {
        let local = routing_node("node-a", "192.0.2.1", Some("10.0.1.0/24"), Some("rack-a"));
        let remote = routing_node("node-b", "192.0.2.2", Some("10.0.2.0/24"), Some("rack-a"));
        let config = RouteConfig::new(Some("router.example.com/group".into()));
        let mock = SubnetLookupMock { addresses: None };

        assert_eq!(
            get_route_mode(&local, &remote, &config, &mock),
            RouteMode::Ipip
        );
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

    struct ReconcileMock {
        tunnel_exists: bool,
        calls: Mutex<Vec<Vec<String>>>,
    }

    impl IpCommandExecutor for ReconcileMock {
        fn run(&self, args: &[&str]) -> io::Result<std::process::Output> {
            self.calls
                .lock()
                .unwrap()
                .push(args.iter().map(|arg| (*arg).to_string()).collect());

            if args.starts_with(&["link", "show", "dev"]) && !self.tunnel_exists {
                return Err(io::Error::new(io::ErrorKind::NotFound, "not found"));
            }

            let stdout = if args == ["-o", "-4", "addr", "show"] {
                b"2: eth0    inet 192.0.2.1/24 brd 192.0.2.255 scope global eth0\n".to_vec()
            } else {
                vec![]
            };
            Ok(std::process::Output {
                status: std::process::ExitStatus::default(),
                stdout,
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
    fn test_reconcile_same_group_installs_direct_route_and_removes_tunnel() {
        let local = routing_node("node-a", "192.0.2.1", Some("10.0.1.0/24"), Some("rack-a"));
        let remote = routing_node("node-b", "192.0.2.2", Some("10.0.2.0/24"), Some("rack-a"));
        let config = RouteConfig::new(Some("router.example.com/group".into()));
        let mock = ReconcileMock {
            tunnel_exists: true,
            calls: Mutex::new(vec![]),
        };
        let tunnel_name = get_tunnel_name("node-b");

        reconcile_route_with_executor(&local, &remote, &config, &mock).unwrap();

        assert_eq!(
            *mock.calls.lock().unwrap(),
            vec![
                vec!["-o", "-4", "addr", "show"],
                vec!["route", "replace", "10.0.2.0/24", "via", "192.0.2.2"],
                vec!["link", "show", "dev", tunnel_name.as_str()],
                vec!["tunnel", "del", tunnel_name.as_str()],
            ]
        );
    }

    #[test]
    fn test_reconcile_different_group_installs_ipip_route() {
        let local = routing_node("node-a", "192.0.2.1", Some("10.0.1.0/24"), Some("rack-a"));
        let remote = routing_node("node-b", "192.0.2.2", Some("10.0.2.0/24"), Some("rack-b"));
        let config = RouteConfig::new(Some("router.example.com/group".into()));
        let mock = ReconcileMock {
            tunnel_exists: false,
            calls: Mutex::new(vec![]),
        };
        let tunnel_name = get_tunnel_name("node-b");

        reconcile_route_with_executor(&local, &remote, &config, &mock).unwrap();

        assert_eq!(
            *mock.calls.lock().unwrap(),
            vec![
                vec!["link", "show", "dev", tunnel_name.as_str()],
                vec![
                    "tunnel",
                    "add",
                    tunnel_name.as_str(),
                    "mode",
                    "ipip",
                    "local",
                    "192.0.2.1",
                    "remote",
                    "192.0.2.2",
                ],
                vec!["link", "set", "dev", tunnel_name.as_str(), "up"],
                vec!["rp_filter", tunnel_name.as_str(), "0"],
                vec![
                    "route",
                    "replace",
                    "10.0.2.0/24",
                    "dev",
                    tunnel_name.as_str()
                ],
            ]
        );
    }

    #[test]
    fn test_reconcile_waits_for_delayed_pod_cidr() {
        let local = routing_node("node-a", "192.0.2.1", Some("10.0.1.0/24"), Some("rack-a"));
        let mut remote = routing_node("node-b", "192.0.2.2", None, Some("rack-b"));
        let config = RouteConfig::new(Some("router.example.com/group".into()));
        let mock = ReconcileMock {
            tunnel_exists: false,
            calls: Mutex::new(vec![]),
        };

        reconcile_route_with_executor(&local, &remote, &config, &mock).unwrap();
        assert!(mock.calls.lock().unwrap().is_empty());

        remote.spec.as_mut().unwrap().pod_cidr = Some("10.0.2.0/24".to_string());
        reconcile_route_with_executor(&local, &remote, &config, &mock).unwrap();

        let expected_route = vec![
            "route".to_string(),
            "replace".to_string(),
            "10.0.2.0/24".to_string(),
            "dev".to_string(),
            get_tunnel_name("node-b"),
        ];
        assert!(mock
            .calls
            .lock()
            .unwrap()
            .iter()
            .any(|args| args == &expected_route));
    }

    #[tokio::test]
    async fn test_delete_route_removes_direct_or_ipip_route_and_tunnel() {
        let remote = routing_node("node-b", "192.0.2.2", Some("10.0.2.0/24"), Some("rack-a"));
        let mock = ReconcileMock {
            tunnel_exists: true,
            calls: Mutex::new(vec![]),
        };
        let tunnel_name = get_tunnel_name("node-b");

        delete_route_with_executor(remote, &mock).await;

        assert_eq!(
            *mock.calls.lock().unwrap(),
            vec![
                vec!["route", "del", "10.0.2.0/24"],
                vec!["link", "show", "dev", tunnel_name.as_str()],
                vec!["tunnel", "del", tunnel_name.as_str()],
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
