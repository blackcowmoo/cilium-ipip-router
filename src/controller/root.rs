use super::{builder::ControllerBuilder, handle::ControllerCommand, handle::ControllerHandle};

use futures::{StreamExt, TryStreamExt};
use futures_core::future::BoxFuture;
use k8s_openapi::api::core::v1::Node;
use kube::{
    api::{Api, WatchEvent, WatchParams},
    client::Client,
    ResourceExt,
};
use md5::compute;
use std::{
    future::Future,
    io,
    pin::Pin,
    process::Command,
    task::{Context, Poll},
};
use tokio::time::{self, Duration};

struct IpCommand;

impl IpCommand {
    fn new() -> Self {
        IpCommand
    }

    fn run(&self, args: &[&str]) -> io::Result<std::process::Output> {
        Command::new("ip").args(args).output()
    }
}

// #[derive(CustomResource, Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
// #[kube(group = "cilium.io", version = "v2", kind = "CiliumEndpoint", namespaced)]
// pub struct CiliumEndpointSpec {
//     title: String,
//     content: String,
// }

pub struct Controller {
    handle: ControllerHandle,
    fut: BoxFuture<'static, io::Result<()>>,
}

impl Controller {
    /// Create server build.
    pub(crate) fn builder() -> ControllerBuilder {
        ControllerBuilder::default()
    }

    pub(crate) fn new(builder: ControllerBuilder) -> Self {
        Controller {
            handle: ControllerHandle::new(builder.cmd_tx.clone()),
            fut: Box::pin(ControllerInner::watch(builder)),
        }
    }

    /// Get a `Server` handle that can be used issue commands and change it's state.
    ///
    /// See [ServerHandle](ServerHandle) for usage.
    pub fn handle(&self) -> ControllerHandle {
        self.handle.clone()
    }
}

impl Future for Controller {
    type Output = io::Result<()>;

    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut Pin::into_inner(self).fut).poll(cx)
    }
}

pub async fn run() -> Controller {
    log::info!("start controller");
    Controller::new(Controller::builder())
}

pub struct ControllerInner {}

impl ControllerInner {
    pub fn get_tunnel_name(node_name: &str) -> String {
        let hash = compute(node_name);
        let hex_hash = format!("{:x}", hash);
        let truncated_hash = &hex_hash[0..11];
        format!("tun-{}", truncated_hash)
    }

    fn run_ip_command(args: &[&str]) -> io::Result<std::process::Output> {
        IpCommand::new().run(args)
    }

    fn tunnel_exists(tunnel_name: &str) -> io::Result<bool> {
        let output = IpCommand::new().run(&["tunnel", "show", tunnel_name])?;
        Ok(output.status.success())
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

    pub async fn watch(mut builder: ControllerBuilder) -> io::Result<()> {
        let client = Client::try_default()
            .await
            .expect("failed to create kube Client");
        let nodes: Api<Node> = Api::all(client);
        let lp = WatchParams::default();

        let mut stream = nodes
            .watch(&lp, "0")
            .await
            .expect("failed to watch nodes")
            .boxed();

        let mut tick = time::interval(Duration::from_secs(1));

        loop {
            tokio::select! {
                Ok(Some(status)) = stream.try_next() => {
                    match status {
                        WatchEvent::Added(node) |
                        WatchEvent::Modified(node)  => {
                            Self::update_route(node).await;
                        },
                        WatchEvent::Deleted(node) => {
                            Self::delete_route(node).await;
                        },
                         WatchEvent::Bookmark(_s) => {},
                         WatchEvent::Error(s) => println!("{}", s),
                    }
                },
                _ = tick.tick()  => {
                    if let Ok(sig) = builder.cmd_rx.try_recv() {
                        match sig {
                            ControllerCommand::Stop { graceful } => {
                                if graceful {
                                    log::info!("shutdown controller");
                                    break;
                                }
                            }
                        }
                    }
                }
            };
        }

        Ok(())
    }

    async fn update_route(node: Node) {
        let node_name = node.name_any();
        let node_ip = Self::get_node_ip(&node);

        match node_ip {
            Some(ip) => {
                let tunnel_name = Self::get_tunnel_name(&node_name);

                if let Ok(true) = Self::tunnel_exists(&tunnel_name) {
                    log::info!("Tunnel {} already exists, skipping creation", tunnel_name);
                    return;
                }

                if let Ok(output) = Self::run_ip_command(&[
                    "tunnel",
                    "add",
                    &tunnel_name,
                    "mode",
                    "ipip",
                    "remote",
                    &ip,
                ]) {
                    if !output.status.success() {
                        log::error!("Failed to create tunnel {}: command failed", tunnel_name);
                        return;
                    }
                } else {
                    log::error!("Failed to create tunnel {}: command error", tunnel_name);
                    return;
                }

                log::info!(
                    "Created IPIP tunnel {} for node {} with remote IP {}",
                    tunnel_name,
                    node_name,
                    ip
                );
            }
            None => {
                log::warn!("No IP address found for node {}", node_name);
            }
        }
    }

    async fn delete_route(node: Node) {
        let node_name = node.name_any();
        let tunnel_name = Self::get_tunnel_name(&node_name);

        if let Ok(output) = Self::run_ip_command(&["tunnel", "del", &tunnel_name]) {
            if output.status.success() {
                log::info!("Deleted IPIP tunnel {} for node {}", tunnel_name, node_name);
            } else {
                log::error!("Failed to delete tunnel {}: command failed", tunnel_name);
            }
        } else {
            log::error!("Failed to delete tunnel {}: command error", tunnel_name);
        }
    }
}
