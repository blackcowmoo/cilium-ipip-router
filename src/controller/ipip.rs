pub use k8s_openapi::api::core::v1::Node;
use kube::client::Client;
use std::{future::Future, io, pin::Pin, process::Command, task::{Context, Poll}};

use crate::controller::root::ControllerInner;

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

pub fn get_tunnel_name(node_name: &str) -> String {
    use md5::compute;
    let hash = compute(node_name);
    let hex_hash = format!("{:x}", hash);
    let truncated_hash = &hex_hash[0..11];
    format!("tun-{}", truncated_hash)
}

pub fn tunnel_exists<T: IpCommandExecutor>(
    executor: &T,
    tunnel_name: &str,
) -> io::Result<bool> {
    match executor.run(&["tunnel", "show", tunnel_name]) {
        Ok(output) => Ok(output.status.success()),
        Err(_) => Ok(false),
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

pub async fn get_local_node_ip() -> Option<String> {
    let hostname = std::env::var("HOSTNAME").ok()?;
    let client = Client::try_default().await.ok()?;
    let nodes: kube::Api<Node> = kube::Api::all(client);

    nodes
        .list(&Default::default())
        .await
        .ok()?
        .into_iter()
        .find(|n| n.metadata.name.as_deref() == Some(hostname.as_str()))
        .and_then(|node| get_node_ip(&node))
}

pub async fn update_route_with_executor<T: IpCommandExecutor>(node: Node, executor: &T) {
    let node_name = node.name_any();
    let node_ip = get_node_ip(&node);
    let node_cidr = get_node_cidr(&node);

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

            if let (Some(cidr), Some(_ip)) = (node_cidr, node_ip) {
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
