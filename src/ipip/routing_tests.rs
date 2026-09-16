use super::*;
use crate::test_support::{config, node, Network, Route};
use std::os::unix::process::ExitStatusExt;

const CIDR: &str = "10.244.2.0/24";

fn peers() -> (Node, Node) {
    (
        node("local", "192.0.2.1", Some("10.244.1.0/24"), Some("a")),
        node("remote", "192.0.2.2", Some(CIDR), Some("a")),
    )
}

#[test]
fn prefix_lookup_matches_exact_internal_ip_among_multiple_interfaces() {
    let output = b"1: lo inet 127.0.0.1/8 scope host lo\n\
        2: eth0 inet 192.0.2.10/16 scope global eth0\n\
        3: eth1 inet 192.0.2.1/25 brd 192.0.2.127 scope global eth1\n\
        4: cilium_host inet 10.244.1.1/32 scope global cilium_host\n";
    assert_eq!(get_local_ipv4_prefix(output, "192.0.2.1"), Some(25));
    assert_eq!(get_local_ipv4_prefix(output, "192.0.2.2"), None);
}

#[test]
fn prefix_lookup_rejects_missing_or_malformed_addresses() {
    for output in [
        "",
        "2: eth0 inet",
        "2: eth0 inet 192.0.2.1",
        "2: eth0 inet invalid/24",
        "2: eth0 inet 192.0.2.1/invalid",
        "2: eth0 inet 192.0.2.1/33",
        "2: eth0 inet 192.0.2.1/256",
        "2: eth0 inet6 2001:db8::1/64",
    ] {
        assert_eq!(
            get_local_ipv4_prefix(output.as_bytes(), "192.0.2.1"),
            None,
            "{output}"
        );
    }
    assert_eq!(
        get_local_ipv4_prefix(b"2: eth0 inet 192.0.2.1/24", "invalid"),
        None
    );
    assert_eq!(
        get_local_ipv4_prefix(b"2: eth0 inet 192.0.2.1/24", "2001:db8::1"),
        None
    );
}

#[test]
fn subnet_comparison_respects_non_octet_prefixes_and_boundaries() {
    for (local, remote, prefix, expected) in [
        ("192.0.2.1", "192.0.2.126", 25, true),
        ("192.0.2.1", "192.0.2.128", 25, false),
        ("10.0.0.1", "10.0.1.254", 23, true),
        ("10.0.0.1", "10.0.2.1", 23, false),
        ("192.0.2.0", "192.0.2.1", 31, true),
        ("192.0.2.0", "192.0.2.2", 31, false),
        ("192.0.2.1", "192.0.2.1", 32, true),
        ("192.0.2.1", "192.0.2.2", 32, false),
        ("192.0.2.1", "198.51.100.1", 0, true),
        ("192.0.2.1", "192.0.2.1", 33, false),
        ("invalid", "192.0.2.2", 24, false),
        ("192.0.2.1", "invalid", 24, false),
        ("2001:db8::1", "2001:db8::2", 24, false),
    ] {
        assert_eq!(
            is_same_ipv4_subnet(local, remote, prefix),
            expected,
            "{local}/{prefix} -> {remote}"
        );
    }
}

#[test]
fn routing_uses_local_interface_prefix_not_pod_cidr_or_other_interface_subnet() {
    let (local, mut remote) = peers();
    let network = Network {
        addresses: "2: eth0 inet 192.0.2.10/16\n3: eth1 inet 192.0.2.1/25\n".into(),
        ..Default::default()
    };
    remote.status.as_mut().unwrap().addresses.as_mut().unwrap()[0].address = "192.0.2.128".into();
    // Matching PodCIDRs do not authorize direct routing outside the node subnet.
    remote.spec = local.spec.clone();
    assert_eq!(
        get_route_mode(&local, &remote, &config(), &network),
        RouteMode::Ipip
    );
    remote.status.as_mut().unwrap().addresses.as_mut().unwrap()[0].address = "192.0.2.126".into();
    assert_eq!(
        get_route_mode(&local, &remote, &config(), &network),
        RouteMode::Direct
    );
}

#[test]
fn missing_or_invalid_local_prefix_falls_back_to_ipip() {
    let (local, remote) = peers();
    for addresses in [
        "",
        "2: eth0 inet 192.0.2.10/24",
        "2: eth0 inet 192.0.2.1/33",
    ] {
        let network = Network {
            addresses: addresses.into(),
            ..Default::default()
        };
        assert_eq!(
            get_route_mode(&local, &remote, &config(), &network),
            RouteMode::Ipip
        );
    }
}

#[test]
fn direct_routing_requires_internal_ips_on_both_nodes() {
    for local_missing in [true, false] {
        let (mut local, mut remote) = peers();
        let node = if local_missing {
            &mut local
        } else {
            &mut remote
        };
        node.status.as_mut().unwrap().addresses.as_mut().unwrap()[0].type_ = "ExternalIP".into();
        let network = Network::default();
        assert_eq!(
            get_route_mode(&local, &remote, &config(), &network),
            RouteMode::Ipip
        );
        assert!(network.calls.borrow().is_empty());
    }
}

#[test]
fn group_policy_covers_missing_empty_and_different_labels_in_both_directions() {
    for (a, b, expected) in [
        (Some("a"), Some("a"), RouteMode::Direct),
        (Some("a"), Some("b"), RouteMode::Ipip),
        (None, None, RouteMode::Direct),
        (Some(""), None, RouteMode::Direct),
        (Some("a"), None, RouteMode::Ipip),
        (None, Some("a"), RouteMode::Ipip),
    ] {
        let local = node("local", "192.0.2.1", None, a);
        let remote = node("remote", "192.0.2.2", Some(CIDR), b);
        let network = Network::default();
        assert_eq!(
            get_route_mode(&local, &remote, &config(), &network),
            expected
        );
    }
    let (local, remote) = peers();
    let network = Network::default();
    assert_eq!(
        get_route_mode(&local, &remote, &RouteConfig::default(), &network),
        RouteMode::Ipip
    );
    assert!(network.calls.borrow().is_empty());
}

#[test]
fn no_commands_for_self_missing_remote_ip_or_missing_local_tunnel_endpoint() {
    let (local, remote) = peers();
    let network = Network::default();
    reconcile_route_with_executor(&local, &local, &config(), &network).unwrap();
    let mut no_ip = remote.clone();
    no_ip.status = None;
    reconcile_route_with_executor(&local, &no_ip, &config(), &network).unwrap();
    let mut no_local_ip = local;
    no_local_ip.status = None;
    reconcile_route_with_executor(&no_local_ip, &remote, &RouteConfig::default(), &network)
        .unwrap();
    assert!(network.calls.borrow().is_empty());
}

#[test]
fn different_subnet_installs_tunnel_with_correct_endpoints() {
    let (local, _) = peers();
    let remote = node("remote", "198.51.100.2", Some(CIDR), Some("a"));
    let network = Network::default();
    reconcile_route_with_executor(&local, &remote, &config(), &network).unwrap();
    let name = get_tunnel_name("remote");
    assert_eq!(network.route(CIDR), Some(Route::Tunnel(name.clone())));
    let tunnels = network.tunnels.borrow();
    assert_eq!(tunnels[&name].local, "192.0.2.1");
    assert_eq!(tunnels[&name].remote, "198.51.100.2");
    assert!(tunnels[&name].up && tunnels[&name].rp_filter_disabled);
}

#[test]
fn failed_direct_route_replacement_preserves_working_ipip_route() {
    let (local, remote) = peers();
    let mut network = Network::default();
    reconcile_route_with_executor(&local, &remote, &RouteConfig::default(), &network).unwrap();
    network.fail_on = Some(
        vec!["route", "replace", CIDR, "via", "192.0.2.2"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
    );
    assert!(reconcile_route_with_executor(&local, &remote, &config(), &network).is_err());
    assert_eq!(
        network.route(CIDR),
        Some(Route::Tunnel(get_tunnel_name("remote")))
    );
    assert_eq!(network.tunnels.borrow().len(), 1);
}

#[test]
fn tunnel_setup_failures_preserve_working_direct_route() {
    let (local, remote) = peers();
    let name = get_tunnel_name("remote");
    for command in [
        vec![
            "tunnel",
            "add",
            &name,
            "mode",
            "ipip",
            "local",
            "192.0.2.1",
            "remote",
            "192.0.2.2",
        ],
        vec!["link", "set", "dev", &name, "up"],
        vec!["rp_filter", &name, "0"],
        vec!["route", "replace", CIDR, "dev", &name],
    ] {
        let mut network = Network::default();
        reconcile_route_with_executor(&local, &remote, &config(), &network).unwrap();
        network.fail_on = Some(command.iter().map(|arg| (*arg).to_owned()).collect());
        let error =
            reconcile_route_with_executor(&local, &remote, &RouteConfig::default(), &network)
                .unwrap_err();
        assert_eq!(error.to_string(), "injected command failure");
        assert_eq!(
            network.route(CIDR),
            Some(Route::Direct("192.0.2.2".into())),
            "{command:?}"
        );
        assert_eq!(network.calls.borrow().last(), network.fail_on.as_ref());
    }
}

#[test]
fn tunnel_cleanup_failure_is_reported_after_direct_route_is_installed() {
    let (local, remote) = peers();
    let mut network = Network::default();
    reconcile_route_with_executor(&local, &remote, &RouteConfig::default(), &network).unwrap();
    network.fail_on = Some(vec![
        "tunnel".into(),
        "del".into(),
        get_tunnel_name("remote"),
    ]);
    assert!(reconcile_route_with_executor(&local, &remote, &config(), &network).is_err());
    assert_eq!(network.route(CIDR), Some(Route::Direct("192.0.2.2".into())));
    assert_eq!(network.tunnels.borrow().len(), 1);
}

#[tokio::test]
async fn deletion_does_not_require_remote_status_or_existing_route() {
    let (local, mut remote) = peers();
    let network = Network::default();
    reconcile_route_with_executor(&local, &remote, &RouteConfig::default(), &network).unwrap();
    remote.status = None;
    network.routes.borrow_mut().clear();
    delete_route_with_executor(remote, &network).await;
    assert!(network.tunnels.borrow().is_empty());
    assert!(network.routes.borrow().is_empty());
}

#[tokio::test]
async fn deletion_without_cidr_still_cleans_up_tunnel() {
    let (local, mut remote) = peers();
    let network = Network::default();
    reconcile_route_with_executor(&local, &remote, &RouteConfig::default(), &network).unwrap();
    remote.spec = None;
    delete_route_with_executor(remote, &network).await;
    assert!(network.tunnels.borrow().is_empty());
    assert!(network.routes.borrow().is_empty());
}

#[tokio::test]
async fn deletion_keeps_remaining_tunnel_when_cleanup_command_fails() {
    let (local, remote) = peers();
    let mut network = Network::default();
    reconcile_route_with_executor(&local, &remote, &RouteConfig::default(), &network).unwrap();
    network.fail_on = Some(vec![
        "tunnel".into(),
        "del".into(),
        get_tunnel_name("remote"),
    ]);
    delete_route_with_executor(remote, &network).await;
    assert!(network.routes.borrow().is_empty());
    assert_eq!(network.tunnels.borrow().len(), 1);
}

struct FailedAddressCommand;

impl IpCommandExecutor for FailedAddressCommand {
    fn run(&self, args: &[&str]) -> io::Result<std::process::Output> {
        assert_eq!(args, ["-o", "-4", "addr", "show"]);
        Ok(std::process::Output {
            status: std::process::ExitStatus::from_raw(256),
            // A plausible stdout must not be trusted when the command failed.
            stdout: b"2: eth0 inet 192.0.2.1/24".to_vec(),
            stderr: vec![],
        })
    }
    fn disable_reverse_path_filter(&self, _: &str) -> io::Result<()> {
        panic!("unexpected rp_filter change")
    }
}

#[test]
fn unsuccessful_address_command_falls_back_to_ipip() {
    let (local, remote) = peers();
    assert_eq!(
        get_route_mode(&local, &remote, &config(), &FailedAddressCommand),
        RouteMode::Ipip
    );
}
