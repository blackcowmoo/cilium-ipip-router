use super::node_routes::NodeRoutes;
use super::{builder::ControllerBuilder, handle::ControllerCommand, handle::ControllerHandle};
use crate::ipip::executor::{get_local_node_name, IpCommand, RouteConfig};
use crate::ipip::Node;

use futures::{StreamExt, TryStreamExt};
use futures_core::future::BoxFuture;
use kube::{
    api::{Api, WatchParams},
    client::Client,
};
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::time::{self, Duration};

pub struct Controller {
    handle: ControllerHandle,
    fut: BoxFuture<'static, io::Result<()>>,
}

impl Controller {
    /// Create server build.
    pub fn builder() -> ControllerBuilder {
        ControllerBuilder::default()
    }

    pub fn new(builder: ControllerBuilder) -> Self {
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
        crate::ipip::executor::get_tunnel_name(node_name)
    }

    pub fn get_node_ip(node: &Node) -> Option<String> {
        crate::ipip::executor::get_node_ip(node)
    }

    pub fn get_node_cidr(node: &Node) -> Option<String> {
        crate::ipip::executor::get_node_cidr(node)
    }

    pub fn tunnel_exists<T: crate::ipip::executor::IpCommandExecutor>(
        executor: &T,
        tunnel_name: &str,
    ) -> io::Result<bool> {
        crate::ipip::executor::tunnel_exists(executor, tunnel_name)
    }

    pub async fn watch(mut builder: ControllerBuilder) -> io::Result<()> {
        let client = match Client::try_default().await {
            Ok(c) => c,
            Err(e) => {
                log::error!("failed to create kube Client: {}", e);
                return Err(io::Error::new(
                    io::ErrorKind::ConnectionRefused,
                    "Kubernetes client unavailable",
                ));
            }
        };
        let nodes: Api<Node> = Api::all(client);
        let lp = WatchParams::default();

        let node_list = match nodes.list(&Default::default()).await {
            Ok(list) => list,
            Err(e) => {
                log::error!("failed to list nodes: {}", e);
                return Err(io::Error::new(
                    io::ErrorKind::ConnectionRefused,
                    "Kubernetes node list unavailable",
                ));
            }
        };
        let resource_version = node_list
            .metadata
            .resource_version
            .clone()
            .unwrap_or_else(|| "0".to_string());

        let mut stream = match nodes.watch(&lp, &resource_version).await {
            Ok(s) => s.boxed(),
            Err(e) => {
                log::error!("failed to watch nodes: {}", e);
                return Err(io::Error::new(
                    io::ErrorKind::ConnectionRefused,
                    "Kubernetes watch unavailable",
                ));
            }
        };

        let mut tick = time::interval(Duration::from_secs(1));
        let Some(local_node_name) = get_local_node_name() else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "NODE_NAME or HOSTNAME must identify the local Kubernetes node",
            ));
        };
        let route_config = RouteConfig::from_env();
        let executor = IpCommand::new();

        match route_config.node_group_label() {
            Some(label) => log::info!("Using node group label {} for route selection", label),
            None => log::info!("NODE_GROUP_LABEL is not set; all remote nodes use IPIP"),
        }

        let mut routes = NodeRoutes::new(local_node_name, route_config, node_list.items);
        routes.reconcile_all(&executor)?;

        loop {
            tokio::select! {
                Ok(Some(status)) = stream.try_next() => {
                    routes.handle_event(status, &executor).await;
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
}
