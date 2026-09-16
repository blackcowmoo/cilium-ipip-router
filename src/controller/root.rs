use super::{builder::ControllerBuilder, handle::ControllerCommand, handle::ControllerHandle};
use crate::ipip::executor::{
    delete_route_with_executor, get_local_node_name, get_node_cidr, reconcile_route_with_executor,
    IpCommand, RouteConfig,
};
use crate::ipip::Node;

use futures::{StreamExt, TryStreamExt};
use futures_core::future::BoxFuture;
use kube::{
    api::{Api, WatchEvent, WatchParams},
    client::Client,
};
use std::collections::HashMap;
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
        let mut known_nodes = node_list
            .items
            .into_iter()
            .filter_map(|node| node.metadata.name.clone().map(|name| (name, node)))
            .collect::<HashMap<String, Node>>();

        match route_config.node_group_label() {
            Some(label) => log::info!("Using node group label {} for route selection", label),
            None => log::info!("NODE_GROUP_LABEL is not set; all remote nodes use IPIP"),
        }

        let Some(local_node) = known_nodes.get(&local_node_name) else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("local Kubernetes node {local_node_name} was not found"),
            ));
        };
        for remote_node in known_nodes.values() {
            if let Err(error) =
                reconcile_route_with_executor(local_node, remote_node, &route_config, &executor)
            {
                log::error!(
                    "Failed to reconcile initial route for node {}: {}",
                    remote_node.metadata.name.as_deref().unwrap_or("<unknown>"),
                    error
                );
            }
        }

        loop {
            tokio::select! {
                Ok(Some(status)) = stream.try_next() => {
                    match status {
                        WatchEvent::Added(node) |
                        WatchEvent::Modified(node)  => {
                            let node_name = node.metadata.name.clone().unwrap_or_default();
                            if node_name.is_empty() {
                                log::warn!("Ignoring node event without metadata.name");
                                continue;
                            }

                            if let Some(previous) = known_nodes.get(&node_name) {
                                if node_name != local_node_name
                                    && get_node_cidr(previous) != get_node_cidr(&node)
                                    && get_node_cidr(previous).is_some()
                                {
                                    delete_route_with_executor(previous.clone(), &executor).await;
                                }
                            }
                            known_nodes.insert(node_name.clone(), node.clone());

                            let Some(local_node) = known_nodes.get(&local_node_name) else {
                                log::debug!(
                                    "Waiting for local node {} before reconciling {}",
                                    local_node_name,
                                    node_name
                                );
                                continue;
                            };

                            if node_name == local_node_name {
                                for remote_node in known_nodes.values() {
                                    if let Err(error) = reconcile_route_with_executor(
                                        local_node,
                                        remote_node,
                                        &route_config,
                                        &executor,
                                    ) {
                                        log::error!(
                                            "Failed to reconcile route for node {}: {}",
                                            remote_node.metadata.name.as_deref().unwrap_or("<unknown>"),
                                            error
                                        );
                                    }
                                }
                            } else if let Err(error) = reconcile_route_with_executor(
                                local_node,
                                &node,
                                &route_config,
                                &executor,
                            ) {
                                log::error!(
                                    "Failed to reconcile route for node {}: {}",
                                    node_name,
                                    error
                                );
                            }
                        },
                        WatchEvent::Deleted(node) => {
                            let node_name = node.metadata.name.clone().unwrap_or_default();
                            known_nodes.remove(&node_name);
                            if node_name != local_node_name {
                                delete_route_with_executor(node, &executor).await;
                            }
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
}
