use super::{builder::ControllerBuilder, handle::ControllerHandle};
use crate::controller::handle::ControllerCommand;

use futures::{StreamExt, TryStreamExt};
use futures_core::future::BoxFuture;
use k8s_openapi::api::core::v1::Node;
use kube::{
    api::{Api, WatchEvent, WatchParams},
    client::Client,
    ResourceExt,
};
use std::{
    future::Future,
    io,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::time::{self, Duration};

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

struct ControllerInner {}

impl ControllerInner {
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
                         WatchEvent::Bookmark(s) => {},
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
        log::info!("Applied: {}", node.name_any());
    }

    async fn delete_route(node: Node) {
        log::info!("Deleted: {}", node.name_any());
    }
}
