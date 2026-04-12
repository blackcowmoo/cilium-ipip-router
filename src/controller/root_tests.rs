use crate::controller::builder::ControllerBuilder;
use crate::controller::handle::ControllerHandle;
use k8s_openapi::api::core::v1::Node;

#[tokio::test]
async fn test_controller_builder_new() {
    let builder = ControllerBuilder::new();
    let _ = builder.cmd_tx;
}

#[tokio::test]
async fn test_controller_builder_default() {
    let builder = ControllerBuilder::default();
    let _ = builder.cmd_tx;
}

#[tokio::test]
async fn test_controller_handle_new() {
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let handle = ControllerHandle::new(tx);
    let _ = handle;
}

#[tokio::test]
async fn test_controller_handle_stop_graceful() {
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let handle = ControllerHandle::new(tx);
    handle.stop(true).await;
}

#[tokio::test]
async fn test_controller_handle_stop_non_graceful() {
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let handle = ControllerHandle::new(tx);
    handle.stop(false).await;
}

#[test]
fn test_controller_handle_clone() {
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let handle = ControllerHandle::new(tx.clone());
    let _ = handle.clone();
}

#[tokio::test]
async fn test_controller_inner_update_route() {
    let node = Node::default();
    crate::controller::root::ControllerInner::update_route(node).await;
}

#[tokio::test]
async fn test_controller_inner_delete_route() {
    let node = Node::default();
    crate::controller::root::ControllerInner::delete_route(node).await;
}
