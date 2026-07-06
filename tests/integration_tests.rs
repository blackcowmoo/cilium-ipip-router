use k8s_openapi::api::core::v1::Node;
use kube::{api::WatchParams, Api, Client};

#[tokio::test]
#[ignore = "requires Kubernetes cluster connection"]
async fn test_controller_connectivity() -> anyhow::Result<()> {
    let client = Client::try_default()
        .await
        .inspect_err(|e| log::error!("Failed to create Kubernetes client: {}", e))?;

    let nodes: Api<Node> = Api::all(client.clone());

    let node_list = nodes.list(&Default::default()).await.inspect_err(|e| {
        log::error!("Failed to list nodes: {}", e);
    })?;

    log::info!("Found {} nodes in cluster", node_list.items.len());

    Ok(())
}

#[tokio::test]
#[ignore = "requires Kubernetes cluster connection"]
async fn test_node_watch_connection() -> anyhow::Result<()> {
    let client = Client::try_default()
        .await
        .inspect_err(|e| log::error!("Failed to create Kubernetes client: {}", e))?;

    let nodes: Api<Node> = Api::all(client.clone());

    let lp = WatchParams::default();

    let _stream = nodes.watch(&lp, "0").await.inspect_err(|e| {
        log::error!("Failed to setup watch stream: {}", e);
    })?;

    Ok(())
}

#[tokio::test]
#[ignore = "requires Kubernetes cluster connection - use E2E tests for actual tunnel/route verification"]
async fn test_tunnels_exist_across_nodes() -> anyhow::Result<()> {
    // This test is a placeholder - actual tunnel verification requires executing
    // commands inside the Kind worker nodes, not from the CI runner container.
    // See tests/e2e/test_tunnels.sh for the proper implementation.
    
    log::info!("Tunnel verification is implemented in E2E tests");
    log::info!("Run: bash tests/e2e/run.sh");
    
    Ok(())
}

#[tokio::test]
#[ignore = "requires Kubernetes cluster connection - use E2E tests for actual route verification"]
async fn test_routes_exist_across_nodes() -> anyhow::Result<()> {
    // This test is a placeholder - actual route verification requires executing
    // commands inside the Kind worker nodes, not from the CI runner container.
    // See tests/e2e/test_routes.sh for the proper implementation.
    
    log::info!("Route verification is implemented in E2E tests");
    log::info!("Run: bash tests/e2e/run.sh");
    
    Ok(())
}

#[tokio::test]
#[ignore = "requires Kubernetes cluster connection - use E2E tests for actual verification"]
async fn test_tunnels_and_routes_across_all_nodes() -> anyhow::Result<()> {
    // This test is a placeholder - actual tunnel/route verification requires executing
    // commands inside the Kind worker nodes, not from the CI runner container.
    // See tests/e2e/test_tunnels.sh and tests/e2e/test_routes.sh for proper implementation.
    
    log::info!("Tunnel and route verification is implemented in E2E tests");
    log::info!("Run: bash tests/e2e/run.sh");
    
    Ok(())
}
