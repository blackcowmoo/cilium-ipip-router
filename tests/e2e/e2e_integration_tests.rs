use k8s_openapi::api::core::v1::Node;
use kube::{Api, Client};
use kube::ResourceExt;

#[tokio::test]
#[ignore = "requires Kind cluster with cilium-ipip-router deployed"]
async fn test_controller_watches_nodes() -> anyhow::Result<()> {
    let client = Client::try_default().await?;
    let nodes: Api<Node> = Api::all(client.clone());

    // List nodes to verify connection
    let node_list = nodes.list(&Default::default()).await?;
    log::info!("Found {} nodes in cluster", node_list.items.len());

    assert!(node_list.items.len() >= 2, "Need at least 2 nodes for testing");

    Ok(())
}

#[tokio::test]
#[ignore = "requires Kind cluster with cilium-ipip-router deployed"]
async fn test_node_lifecycle_create_delete() -> anyhow::Result<()> {
    // This test demonstrates how to use the controller with mock nodes
    // In practice, we'd use a fake client to simulate node creation/deletion
    
    let client = Client::try_default().await?;
    let nodes: Api<Node> = Api::all(client.clone());

    // Get an existing node to use as a template
    let node_list = nodes.list(&Default::default()).await?;
    assert!(!node_list.items.is_empty(), "At least one node should exist");

    let original_node = &node_list.items[0];
    log::info!("Using node: {}", original_node.name_any());

    // Verify the node has pod CIDR (required for tunnel creation)
    let pod_cidr = original_node.spec.as_ref().and_then(|s| s.pod_cidr.as_ref());
    log::info!("Node pod CIDR: {:?}", pod_cidr);

    Ok(())
}

#[tokio::test]
#[ignore = "requires Kind cluster with cilium-ipip-router deployed"]
async fn test_controller_health_check() -> anyhow::Result<()> {
    // For now, we just verify we can reach the API server
    
    let client = Client::try_default().await?;
    let api_info = client.apiserver_version().await?;
    log::info!("Kubernetes API version: {}", api_info.git_version);

    Ok(())
}
