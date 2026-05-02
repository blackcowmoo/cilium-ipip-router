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
