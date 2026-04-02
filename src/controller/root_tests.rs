use crate::controller::builder::ControllerBuilder;
use k8s_openapi::api::core::v1::Node;

#[tokio::test]
async fn test_controller_builder() {
    let builder = ControllerBuilder::new();
    let _ = builder.cmd_tx;
}
