use super::{builder::ControllerBuilder, handle::ControllerCommand, handle::ControllerHandle};
use crate::controller::ipip::{
    delete_route_with_executor, update_route_with_executor, IpCommand, Node,
};

use futures::{StreamExt, TryStreamExt};
use futures_core::future::BoxFuture;
use kube::{
    api::{Api, WatchEvent, WatchParams},
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
        crate::controller::ipip::get_tunnel_name(node_name)
    }

    pub fn get_node_ip(node: &Node) -> Option<String> {
        crate::controller::ipip::get_node_ip(node)
    }

    pub fn get_node_cidr(node: &Node) -> Option<String> {
        crate::controller::ipip::get_node_cidr(node)
    }

    pub fn tunnel_exists<T: crate::controller::ipip::IpCommandExecutor>(
        executor: &T,
        tunnel_name: &str,
    ) -> io::Result<bool> {
        crate::controller::ipip::tunnel_exists(executor, tunnel_name)
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

        let mut stream = match nodes.watch(&lp, "0").await {
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

        loop {
            tokio::select! {
                Ok(Some(status)) = stream.try_next() => {
                    match status {
                        WatchEvent::Added(node) |
                        WatchEvent::Modified(node)  => {
                            update_route_with_executor(node, &IpCommand::new()).await;
                        },
                        WatchEvent::Deleted(node) => {
                            delete_route_with_executor(node, &IpCommand::new()).await;
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
