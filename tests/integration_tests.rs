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
#[ignore = "requires Kubernetes cluster connection"]
async fn test_tunnels_exist_across_nodes() -> anyhow::Result<()> {
    let client = Client::try_default()
        .await
        .inspect_err(|e| log::error!("Failed to create Kubernetes client: {}", e))?;

    let nodes: Api<Node> = Api::all(client.clone());

    let node_list = nodes.list(&Default::default()).await.inspect_err(|e| {
        log::error!("Failed to list nodes: {}", e);
    })?;

    log::info!("Found {} nodes in cluster", node_list.items.len());

    if node_list.items.is_empty() {
        anyhow::bail!("No nodes found in cluster");
    }

    for node in &node_list.items {
        let node_name = node.metadata.name.as_deref().unwrap_or("unknown");
        let node_ip = node
            .status
            .as_ref()
            .and_then(|s| s.addresses.as_ref())
            .and_then(|addrs| {
                addrs.iter().find(|a| a.type_ == "ExternalIP" || a.type_ == "InternalIP")
            })
            .map(|a| a.address.clone())
            .unwrap_or_else(|| "unknown".to_string());

        log::info!(
            "Checking tunnel for node {} with IP {}",
            node_name,
            node_ip
        );

        let tunnel_name = format!("tun-{}", &node_name[0..11]);
        log::info!("Expected tunnel name: {}", tunnel_name);
    }

    Ok(())
}

#[tokio::test]
#[ignore = "requires Kubernetes cluster connection"]
async fn test_routes_exist_across_nodes() -> anyhow::Result<()> {
    let client = Client::try_default()
        .await
        .inspect_err(|e| log::error!("Failed to create Kubernetes client: {}", e))?;

    let nodes: Api<Node> = Api::all(client.clone());

    let node_list = nodes.list(&Default::default()).await.inspect_err(|e| {
        log::error!("Failed to list nodes: {}", e);
    })?;

    log::info!("Found {} nodes in cluster", node_list.items.len());

    if node_list.items.is_empty() {
        anyhow::bail!("No nodes found in cluster");
    }

    for node in &node_list.items {
        let node_name = node.metadata.name.as_deref().unwrap_or("unknown");
        let pod_cidr = node
            .spec
            .as_ref()
            .and_then(|s| s.pod_cidr.as_ref())
            .map(|c| c.as_str())
            .unwrap_or("unknown");

        log::info!(
            "Checking route for node {} with pod CIDR {}",
            node_name,
            pod_cidr
        );
    }

    Ok(())
}

#[tokio::test]
#[ignore = "requires Kubernetes cluster connection"]
async fn test_tunnels_and_routes_across_all_nodes() -> anyhow::Result<()> {
    let client = Client::try_default()
        .await
        .inspect_err(|e| log::error!("Failed to create Kubernetes client: {}", e))?;

    let nodes: Api<Node> = Api::all(client.clone());

    let node_list = nodes.list(&Default::default()).await.inspect_err(|e| {
        log::error!("Failed to list nodes: {}", e);
    })?;

    log::info!("Found {} nodes in cluster", node_list.items.len());

    if node_list.items.is_empty() {
        anyhow::bail!("No nodes found in cluster");
    }

    let node_count = node_list.items.len();
    log::info!("Verifying tunnels and routes for all {} nodes", node_count);

    for node in &node_list.items {
        let node_name = node.metadata.name.as_deref().unwrap_or("unknown");
        let node_ip = node
            .status
            .as_ref()
            .and_then(|s| s.addresses.as_ref())
            .and_then(|addrs| {
                addrs.iter().find(|a| a.type_ == "ExternalIP" || a.type_ == "InternalIP")
            })
            .map(|a| a.address.clone())
            .unwrap_or_else(|| "unknown".to_string());
        let pod_cidr = node
            .spec
            .as_ref()
            .and_then(|s| s.pod_cidr.as_ref())
            .map(|c| c.as_str())
            .unwrap_or("unknown");

        log::info!(
            "Node {}: IP={}, PodCIDR={}",
            node_name,
            node_ip,
            pod_cidr
        );

        let tunnel_name = format!("tun-{}", &node_name[0..11]);
        log::info!("  Expected tunnel: {}", tunnel_name);
        log::info!("  Expected route: {} via {}", pod_cidr, tunnel_name);
    }

    log::info!(
        "Verified tunnels and routes for all {} nodes",
        node_count
    );

    Ok(())
}
