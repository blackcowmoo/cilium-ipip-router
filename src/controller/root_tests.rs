

#[tokio::test]
async fn test_watch_node_added() {
    let builder = ControllerBuilder::new();
    let _controller = ControllerHandle::new(builder.cmd_tx);
}

#[tokio::test]
async fn test_watch_node_modified() {
    let builder = ControllerBuilder::new();
    let _controller = ControllerHandle::new(builder.cmd_tx);
}

#[tokio::test]
async fn test_watch_node_deleted() {
    let builder = ControllerBuilder::new();
    let _controller = ControllerHandle::new(builder.cmd_tx);
}
