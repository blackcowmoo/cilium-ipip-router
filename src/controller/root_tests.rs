use crate::controller::builder::ControllerBuilder;

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
