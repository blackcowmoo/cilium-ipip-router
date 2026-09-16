use crate::ipip::{
    delete_route_with_executor, get_node_cidr, reconcile_route_with_executor, IpCommandExecutor,
    Node, RouteConfig,
};
use kube::api::WatchEvent;
use std::{collections::HashMap, io};

/// Node state shared by initial reconciliation and watch event handling.
/// Kubernetes transport and host commands are supplied by the caller so the
/// same event handling can be exercised without a live cluster or root access.
pub(super) struct NodeRoutes {
    local_node_name: String,
    config: RouteConfig,
    known_nodes: HashMap<String, Node>,
}

impl NodeRoutes {
    pub(super) fn new(local_node_name: String, config: RouteConfig, nodes: Vec<Node>) -> Self {
        Self {
            local_node_name,
            config,
            known_nodes: nodes
                .into_iter()
                .filter_map(|node| node.metadata.name.clone().map(|name| (name, node)))
                .collect(),
        }
    }

    pub(super) fn reconcile_all<T: IpCommandExecutor>(&self, executor: &T) -> io::Result<()> {
        let local_node = self.known_nodes.get(&self.local_node_name).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "local Kubernetes node {} was not found",
                    self.local_node_name
                ),
            )
        })?;
        for remote_node in self.known_nodes.values() {
            self.reconcile(local_node, remote_node, executor);
        }
        Ok(())
    }

    fn reconcile<T: IpCommandExecutor>(&self, local: &Node, remote: &Node, executor: &T) {
        if let Err(error) = reconcile_route_with_executor(local, remote, &self.config, executor) {
            log::error!(
                "Failed to reconcile route for node {}: {}",
                remote.metadata.name.as_deref().unwrap_or("<unknown>"),
                error
            );
        }
    }

    pub(super) async fn handle_event<T: IpCommandExecutor>(
        &mut self,
        event: WatchEvent<Node>,
        executor: &T,
    ) {
        match event {
            WatchEvent::Added(node) | WatchEvent::Modified(node) => {
                let node_name = node.metadata.name.clone().unwrap_or_default();
                if node_name.is_empty() {
                    log::warn!("Ignoring node event without metadata.name");
                    return;
                }
                if let Some(previous) = self.known_nodes.get(&node_name) {
                    if node_name != self.local_node_name
                        && get_node_cidr(previous) != get_node_cidr(&node)
                        && get_node_cidr(previous).is_some()
                    {
                        delete_route_with_executor(previous.clone(), executor).await;
                    }
                }
                self.known_nodes.insert(node_name.clone(), node.clone());
                let Some(local) = self.known_nodes.get(&self.local_node_name) else {
                    log::debug!("Waiting for local node {}", self.local_node_name);
                    return;
                };
                if node_name == self.local_node_name {
                    for remote in self.known_nodes.values() {
                        self.reconcile(local, remote, executor);
                    }
                } else {
                    self.reconcile(local, &node, executor);
                }
            }
            WatchEvent::Deleted(node) => {
                let node_name = node.metadata.name.clone().unwrap_or_default();
                self.known_nodes.remove(&node_name);
                if node_name != self.local_node_name {
                    delete_route_with_executor(node, executor).await;
                }
            }
            WatchEvent::Bookmark(_) => {}
            WatchEvent::Error(status) => log::error!("Node watch error: {}", status),
        }
    }
}

#[cfg(test)]
#[path = "node_routes_tests.rs"]
mod tests;
