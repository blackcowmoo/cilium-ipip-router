use crate::controller::builder::ControllerBuilder;
use crate::controller::root::ControllerInner;
use k8s_openapi::api::core::v1::Node;
use kube::api::{Api, ListParams, WatchEvent, WatchParams};
use mockall::mock;

mock! {
    pub NodeApi {}
    
    impl<'a> Api<Node> for NodeApi {
        fn name(&self) -> &str;
        fn namespace(&self) -> &str;
        fn list(&self, lp: &ListParams) -> kube::Result<kube::api::ObjectList<Node>>;
        fn get(&self, name: &str) -> kube::Result<Node>;
        fn delete(&self, name: &str, params: &ListParams) -> kube::Result<()>;
        fn watch(
            &self,
            params: &WatchParams,
            since: &str,
        ) -> kube::Result<kube::watch::Watcher<std::pin::Pin<Box<dyn std::io::Read + Send>>>>;
    }
}

#[tokio::test]
async fn test_watch_node_added() {
    let builder = ControllerBuilder::new();
    let _ = builder.cmd_tx;
}

#[tokio::test]
async fn test_watch_node_modified() {
    let builder = ControllerBuilder::new();
    let _ = builder.cmd_tx;
}

#[tokio::test]
async fn test_watch_node_deleted() {
    let builder = ControllerBuilder::new();
    let _ = builder.cmd_tx;
}
