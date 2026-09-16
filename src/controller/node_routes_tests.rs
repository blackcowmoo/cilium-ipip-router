use super::*;
use crate::{
    ipip::get_tunnel_name,
    test_support::{config, node, Network, Route},
};

const CIDR: &str = "10.244.2.0/24";

fn local(group: Option<&str>) -> Node {
    node("local", "192.0.2.1", Some("10.244.1.0/24"), group)
}

fn remote(group: Option<&str>) -> Node {
    node("remote", "192.0.2.2", Some(CIDR), group)
}

fn controller(nodes: Vec<Node>) -> NodeRoutes {
    NodeRoutes::new("local".into(), config(), nodes)
}

#[tokio::test]
async fn bookmark_and_watch_error_do_not_change_routes_or_discard_cached_nodes() {
    use kube::core::{
        watch::{Bookmark, BookmarkMeta},
        ErrorResponse, TypeMeta,
    };

    let network = Network::default();
    let mut routes = controller(vec![local(None), remote(None)]);
    routes.reconcile_all(&network).unwrap();
    network.calls.borrow_mut().clear();
    routes
        .handle_event(
            WatchEvent::Bookmark(Bookmark {
                types: TypeMeta {
                    api_version: "v1".into(),
                    kind: "Node".into(),
                },
                metadata: BookmarkMeta {
                    resource_version: "42".into(),
                    annotations: Default::default(),
                },
            }),
            &network,
        )
        .await;
    routes
        .handle_event(
            WatchEvent::Error(ErrorResponse {
                status: "Failure".into(),
                message: "watch expired".into(),
                reason: "Expired".into(),
                code: 410,
            }),
            &network,
        )
        .await;
    assert!(network.calls.borrow().is_empty());
    assert_eq!(network.route(CIDR), Some(Route::Direct("192.0.2.2".into())));
    // The cached peer remains available for subsequent reconciliation.
    routes
        .handle_event(WatchEvent::Modified(local(Some("a"))), &network)
        .await;
    assert_eq!(
        network.route(CIDR),
        Some(Route::Tunnel(get_tunnel_name("remote")))
    );
}

#[tokio::test]
async fn late_group_labels_reconcile_the_unlabeled_default_group() {
    let network = Network::default();
    let mut routes = controller(vec![local(None), remote(None)]);
    routes.reconcile_all(&network).unwrap();
    assert_eq!(network.route(CIDR), Some(Route::Direct("192.0.2.2".into())));
    routes
        .handle_event(WatchEvent::Modified(remote(Some("a"))), &network)
        .await;
    assert_eq!(
        network.route(CIDR),
        Some(Route::Tunnel(get_tunnel_name("remote")))
    );
    routes
        .handle_event(WatchEvent::Modified(local(Some("a"))), &network)
        .await;
    assert_eq!(network.route(CIDR), Some(Route::Direct("192.0.2.2".into())));
    assert!(network.tunnels.borrow().is_empty());
}

#[tokio::test]
async fn subnet_change_is_rechecked_on_the_next_node_event() {
    let mut network = Network::default();
    let peer = node("remote", "192.0.2.129", Some(CIDR), Some("a"));
    let mut routes = controller(vec![local(Some("a")), peer.clone()]);
    routes.reconcile_all(&network).unwrap();
    assert_eq!(
        network.route(CIDR),
        Some(Route::Direct("192.0.2.129".into()))
    );
    network.addresses = "2: eth0 inet 192.0.2.1/25".into();
    routes
        .handle_event(WatchEvent::Modified(peer.clone()), &network)
        .await;
    assert_eq!(
        network.route(CIDR),
        Some(Route::Tunnel(get_tunnel_name("remote")))
    );
    network.addresses = "2: eth0 inet 192.0.2.1/24".into();
    routes
        .handle_event(WatchEvent::Modified(peer), &network)
        .await;
    assert_eq!(
        network.route(CIDR),
        Some(Route::Direct("192.0.2.129".into()))
    );
    assert!(network.tunnels.borrow().is_empty());
}

#[test]
fn startup_reconciles_existing_peers_without_touching_local_routes() {
    let network = Network::default();
    let routes = controller(vec![
        local(Some("a")),
        remote(Some("a")),
        node("other", "198.51.100.2", Some("10.244.3.0/24"), Some("a")),
        Node::default(),
    ]);
    routes.reconcile_all(&network).unwrap();
    assert_eq!(network.route(CIDR), Some(Route::Direct("192.0.2.2".into())));
    assert_eq!(
        network.route("10.244.3.0/24"),
        Some(Route::Tunnel(get_tunnel_name("other")))
    );
    assert_eq!(network.routes.borrow().len(), 2);
    let tunnels = network.tunnels.borrow();
    assert_eq!(tunnels.len(), 1);
    assert_eq!(tunnels[&get_tunnel_name("other")].local, "192.0.2.1");
    assert_eq!(tunnels[&get_tunnel_name("other")].remote, "198.51.100.2");
}

#[test]
fn startup_requires_local_node_and_does_not_modify_network_if_missing() {
    let network = Network::default();
    let error = controller(vec![remote(None)])
        .reconcile_all(&network)
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    assert!(network.calls.borrow().is_empty());
}

#[test]
fn startup_keeps_reconciling_other_peers_after_command_failure() {
    let network = Network {
        fail_on: Some(
            vec!["route", "replace", CIDR, "via", "192.0.2.2"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        ),
        ..Default::default()
    };
    let routes = controller(vec![
        local(None),
        remote(None),
        node("other", "192.0.2.3", Some("10.244.3.0/24"), None),
    ]);
    routes.reconcile_all(&network).unwrap();
    assert_eq!(network.route(CIDR), None);
    assert_eq!(
        network.route("10.244.3.0/24"),
        Some(Route::Direct("192.0.2.3".into()))
    );
}

#[tokio::test]
async fn delayed_pod_cidr_add_modify_delete_lifecycle_cleans_both_routing_modes() {
    for group in [Some("a"), Some("b")] {
        let network = Network::default();
        let mut routes = controller(vec![local(Some("a"))]);
        let mut peer = remote(group);
        peer.spec.as_mut().unwrap().pod_cidr = None;
        routes
            .handle_event(WatchEvent::Added(peer.clone()), &network)
            .await;
        assert!(network.calls.borrow().is_empty());

        peer.spec.as_mut().unwrap().pod_cidr = Some(CIDR.into());
        routes
            .handle_event(WatchEvent::Modified(peer.clone()), &network)
            .await;
        let expected = if group == Some("a") {
            Route::Direct("192.0.2.2".into())
        } else {
            Route::Tunnel(get_tunnel_name("remote"))
        };
        assert_eq!(network.route(CIDR), Some(expected.clone()));

        // A duplicate event must not create duplicate tunnels or lose the route.
        routes
            .handle_event(WatchEvent::Modified(peer.clone()), &network)
            .await;
        assert_eq!(network.routes.borrow().len(), 1);
        assert_eq!(network.route(CIDR), Some(expected));
        routes
            .handle_event(WatchEvent::Deleted(peer.clone()), &network)
            .await;
        assert!(network.routes.borrow().is_empty());
        assert!(network.tunnels.borrow().is_empty());

        // Deleted peers must not be resurrected by a later local-node update.
        routes
            .handle_event(WatchEvent::Modified(local(Some("b"))), &network)
            .await;
        assert!(network.routes.borrow().is_empty());
        routes
            .handle_event(WatchEvent::Deleted(peer), &network)
            .await;
        assert!(network.tunnels.borrow().is_empty());
    }
}

#[tokio::test]
async fn changed_and_removed_pod_cidr_leave_no_old_routes() {
    for group in [Some("a"), Some("b")] {
        let network = Network::default();
        let mut peer = remote(group);
        let mut routes = controller(vec![local(Some("a")), peer.clone()]);
        routes.reconcile_all(&network).unwrap();
        peer.spec.as_mut().unwrap().pod_cidr = Some("10.244.4.0/24".into());
        routes
            .handle_event(WatchEvent::Modified(peer.clone()), &network)
            .await;
        assert_eq!(network.route(CIDR), None);
        assert!(network.route("10.244.4.0/24").is_some());
        assert_eq!(network.routes.borrow().len(), 1);

        peer.spec.as_mut().unwrap().pod_cidr = None;
        routes
            .handle_event(WatchEvent::Modified(peer), &network)
            .await;
        assert!(network.routes.borrow().is_empty());
        assert!(network.tunnels.borrow().is_empty());
    }
}

#[tokio::test]
async fn remote_label_changes_switch_between_direct_and_ipip() {
    let network = Network::default();
    let mut routes = controller(vec![local(Some("a")), remote(Some("a"))]);
    routes.reconcile_all(&network).unwrap();
    assert_eq!(network.route(CIDR), Some(Route::Direct("192.0.2.2".into())));
    routes
        .handle_event(WatchEvent::Modified(remote(Some("b"))), &network)
        .await;
    assert_eq!(
        network.route(CIDR),
        Some(Route::Tunnel(get_tunnel_name("remote")))
    );
    routes
        .handle_event(WatchEvent::Modified(remote(Some("a"))), &network)
        .await;
    assert_eq!(network.route(CIDR), Some(Route::Direct("192.0.2.2".into())));
    assert!(network.tunnels.borrow().is_empty());
}

#[tokio::test]
async fn local_label_changes_reconcile_every_cached_peer() {
    let network = Network::default();
    let mut routes = controller(vec![
        local(Some("a")),
        remote(Some("a")),
        node("other", "192.0.2.3", Some("10.244.3.0/24"), Some("b")),
    ]);
    routes.reconcile_all(&network).unwrap();
    routes
        .handle_event(WatchEvent::Modified(local(Some("b"))), &network)
        .await;
    assert_eq!(
        network.route(CIDR),
        Some(Route::Tunnel(get_tunnel_name("remote")))
    );
    assert_eq!(
        network.route("10.244.3.0/24"),
        Some(Route::Direct("192.0.2.3".into()))
    );
    assert_eq!(network.tunnels.borrow().len(), 1);
    assert!(!network
        .tunnels
        .borrow()
        .contains_key(&get_tunnel_name("other")));
}

#[tokio::test]
async fn peers_wait_for_local_node_then_reconcile_on_arrival() {
    let network = Network::default();
    let mut routes = controller(vec![]);
    routes
        .handle_event(WatchEvent::Added(remote(None)), &network)
        .await;
    assert!(network.calls.borrow().is_empty());
    routes
        .handle_event(WatchEvent::Added(local(None)), &network)
        .await;
    assert_eq!(network.route(CIDR), Some(Route::Direct("192.0.2.2".into())));
}

#[tokio::test]
async fn deleting_local_node_preserves_local_pod_route_and_waits_for_return() {
    let network = Network::default();
    let local_route = Route::Direct("192.0.2.1".into());
    network
        .routes
        .borrow_mut()
        .insert("10.244.1.0/24".into(), local_route.clone());
    let mut routes = controller(vec![local(None)]);
    routes
        .handle_event(WatchEvent::Deleted(local(None)), &network)
        .await;
    routes
        .handle_event(WatchEvent::Added(remote(None)), &network)
        .await;
    assert!(network.calls.borrow().is_empty());
    assert_eq!(network.route("10.244.1.0/24"), Some(local_route));
    routes
        .handle_event(WatchEvent::Added(local(None)), &network)
        .await;
    assert!(network.route(CIDR).is_some());
}

#[tokio::test]
async fn incomplete_node_events_do_not_install_routes() {
    let network = Network::default();
    let mut routes = controller(vec![local(None)]);
    for name in [None, Some(String::new())] {
        let mut peer = remote(None);
        peer.metadata.name = name;
        routes.handle_event(WatchEvent::Added(peer), &network).await;
    }
    let mut peer = remote(None);
    peer.status = None;
    routes.handle_event(WatchEvent::Added(peer), &network).await;
    assert!(network.calls.borrow().is_empty());
    routes
        .handle_event(WatchEvent::Modified(remote(None)), &network)
        .await;
    assert_eq!(network.route(CIDR), Some(Route::Direct("192.0.2.2".into())));
}

#[tokio::test]
async fn reconciliation_failure_preserves_cached_node_for_next_event() {
    let mut network = Network {
        fail_on: Some(
            vec!["route", "replace", CIDR, "via", "192.0.2.2"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        ),
        ..Default::default()
    };
    let mut routes = controller(vec![local(None)]);
    routes
        .handle_event(WatchEvent::Added(remote(None)), &network)
        .await;
    assert_eq!(network.route(CIDR), None);
    network.fail_on = None;
    routes
        .handle_event(WatchEvent::Modified(local(None)), &network)
        .await;
    assert_eq!(network.route(CIDR), Some(Route::Direct("192.0.2.2".into())));
}
