use k8s_openapi::api::core::v1::Node;
use kube::api::{ListParams, WatchEvent};
use kube_core::watch::{Bookmark, BookmarkMeta};
use std::collections::BTreeMap;
use std::io;
use tokio::sync::mpsc::unbounded_channel;

use crate::controller::root::{self, IpCommand, IpCommandExecutor};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controller::builder::ControllerBuilder;
    use crate::controller::handle::{ControllerCommand, ControllerHandle};

    #[test]
    fn test_node_creation() {
        let node = Node {
            metadata: Default::default(),
            spec: Some(k8s_openapi::api::core::v1::NodeSpec {
                external_id: Some("test-node".to_string()),
                ..Default::default()
            }),
            status: Some(k8s_openapi::api::core::v1::NodeStatus {
                node_info: Some(k8s_openapi::api::core::v1::NodeSystemInfo {
                    kernel_version: "test-kernel".to_string(),
                    ..Default::default()
                }),
                ..Default::default()
            }),
        };

        assert_eq!(
            node.spec.unwrap().external_id,
            Some("test-node".to_string())
        );
    }

    #[tokio::test]
    async fn test_controller_builder_integration() {
        let builder = ControllerBuilder::new();
        assert!(builder
            .cmd_tx
            .send(ControllerCommand::Stop { graceful: true })
            .is_ok());
    }

    #[tokio::test]
    async fn test_controller_with_mock_api() {
        let _builder = ControllerBuilder::new();
    }

    #[test]
    fn test_node_status_fields() {
        let node = Node {
            metadata: Default::default(),
            spec: Some(k8s_openapi::api::core::v1::NodeSpec {
                external_id: Some("node-1".to_string()),
                ..Default::default()
            }),
            status: Some(k8s_openapi::api::core::v1::NodeStatus {
                node_info: Some(k8s_openapi::api::core::v1::NodeSystemInfo {
                    kernel_version: "5.15.0".to_string(),
                    ..Default::default()
                }),
                ..Default::default()
            }),
        };

        assert!(node.status.is_some());
        assert!(node.spec.is_some());
    }

    #[test]
    fn test_node_with_empty_status() {
        let node = Node {
            metadata: Default::default(),
            spec: None,
            status: None,
        };

        assert!(node.status.is_none());
        assert!(node.spec.is_none());
    }

    #[test]
    fn test_watch_event_types() {
        let node = Node {
            metadata: Default::default(),
            spec: None,
            status: None,
        };

        let _added = WatchEvent::Added(node.clone());
        let _modified = WatchEvent::Modified(node.clone());
        let _deleted = WatchEvent::Deleted(node);
        let _bookmark: WatchEvent<Node> = WatchEvent::Bookmark(Bookmark {
            types: kube_core::metadata::TypeMeta {
                api_version: "v1".to_string(),
                kind: "Node".to_string(),
            },
            metadata: BookmarkMeta {
                resource_version: "12345".to_string(),
                annotations: BTreeMap::new(),
            },
        });
    }

    #[test]
    fn test_node_spec_fields() {
        let spec = k8s_openapi::api::core::v1::NodeSpec {
            external_id: Some("external-id".to_string()),
            pod_cidr: Some("10.0.0.0/24".to_string()),
            ..Default::default()
        };

        assert_eq!(spec.external_id, Some("external-id".to_string()));
        assert_eq!(spec.pod_cidr, Some("10.0.0.0/24".to_string()));
    }

    #[test]
    fn test_node_status_fields_full() {
        let status = k8s_openapi::api::core::v1::NodeStatus {
            node_info: Some(k8s_openapi::api::core::v1::NodeSystemInfo {
                kernel_version: "5.15.0".to_string(),
                os_image: "Ubuntu 22.04".to_string(),
                container_runtime_version: "docker://20.10.0".to_string(),
                ..Default::default()
            }),
            phase: Some("Running".to_string()),
            ..Default::default()
        };

        assert_eq!(status.node_info.clone().unwrap().kernel_version, "5.15.0");
        assert_eq!(status.node_info.clone().unwrap().os_image, "Ubuntu 22.04");
    }

    #[test]
    fn test_node_api_watch_params() {
        let lp = ListParams::default();
        assert_eq!(lp.timeout, None);
    }

    #[test]
    fn test_node_api_list_params_with_timeout() {
        use kube::api::ListParams;
        let lp = ListParams::timeout(ListParams::default(), 30);
        assert_eq!(lp.timeout, Some(30));
    }

    #[test]
    fn test_controller_builder_default() {
        let builder = ControllerBuilder::default();
        assert!(!builder.cmd_tx.is_closed());
        assert_eq!(builder.cmd_rx.len(), 0);
    }

    #[test]
    fn test_controller_builder_new() {
        let builder = ControllerBuilder::new();
        assert!(!builder.cmd_tx.is_closed());
        assert_eq!(builder.cmd_rx.len(), 0);
    }

    #[test]
    fn test_controller_builder_clone_channel() {
        let builder = ControllerBuilder::new();
        let tx = builder.cmd_tx.clone();
        let _rx = builder.cmd_rx;
        assert!(!tx.is_closed());
    }

    #[tokio::test]
    async fn test_controller_handle_new() {
        let (tx, _rx) = unbounded_channel::<ControllerCommand>();
        let handle = ControllerHandle::new(tx);
        let _debug_str = format!("{:?}", handle);
    }

    #[tokio::test]
    async fn test_controller_handle_stop_graceful() {
        let (tx, mut rx) = unbounded_channel::<ControllerCommand>();
        let handle = ControllerHandle::new(tx);

        handle.stop(true).await;

        let cmd = rx.recv().await.expect("Should receive command");
        match cmd {
            ControllerCommand::Stop { graceful } => assert!(graceful),
        }
    }

    #[tokio::test]
    async fn test_controller_handle_stop_non_graceful() {
        let (tx, mut rx) = unbounded_channel::<ControllerCommand>();
        let handle = ControllerHandle::new(tx);

        handle.stop(false).await;

        let cmd = rx.recv().await.expect("Should receive command");
        match cmd {
            ControllerCommand::Stop { graceful } => assert!(!graceful),
        }
    }

    #[test]
    fn test_controller_command_stop_enum() {
        let stop_graceful = ControllerCommand::Stop { graceful: true };
        let stop_non_graceful = ControllerCommand::Stop { graceful: false };

        matches!(stop_graceful, ControllerCommand::Stop { graceful: true });
        matches!(
            stop_non_graceful,
            ControllerCommand::Stop { graceful: false }
        );
    }

    #[test]
    fn test_controller_handle_debug_impl() {
        let (tx, _rx) = unbounded_channel::<ControllerCommand>();
        let handle = ControllerHandle::new(tx);
        let _debug_str = format!("{:?}", handle);
    }

    #[test]
    fn test_controller_builder_multiple_commands() {
        let builder = ControllerBuilder::new();

        let cmd1 = ControllerCommand::Stop { graceful: true };
        let cmd2 = ControllerCommand::Stop { graceful: false };

        assert!(builder.cmd_tx.send(cmd1).is_ok());
        assert!(builder.cmd_tx.send(cmd2).is_ok());

        assert_eq!(builder.cmd_rx.len(), 2);
    }

    #[test]
    fn test_get_tunnel_name_short() {
        let node_name = "node-1";
        let tunnel_name = crate::controller::root::ControllerInner::get_tunnel_name(node_name);
        assert_eq!(tunnel_name, "tun-d50164b9587");
        assert!(tunnel_name.len() <= 15);
    }

    #[test]
    fn test_get_tunnel_name_long() {
        let node_name = "very-long-node-name-that-exceeds-limit";
        let tunnel_name = crate::controller::root::ControllerInner::get_tunnel_name(node_name);
        assert_eq!(tunnel_name, "tun-743e7336877");
        assert!(tunnel_name.len() <= 15);
    }

    #[test]
    fn test_get_tunnel_name_exact_length() {
        let node_name = "exactly10chars";
        let tunnel_name = crate::controller::root::ControllerInner::get_tunnel_name(node_name);
        assert_eq!(tunnel_name, "tun-15754261d2f");
        assert!(tunnel_name.len() <= 15);
    }

    #[test]
    fn test_get_node_ip_with_external_ip() {
        let node = Node {
            metadata: Default::default(),
            spec: None,
            status: Some(k8s_openapi::api::core::v1::NodeStatus {
                addresses: Some(vec![
                    k8s_openapi::api::core::v1::NodeAddress {
                        type_: "ExternalIP".to_string(),
                        address: "203.0.113.1".to_string(),
                    },
                    k8s_openapi::api::core::v1::NodeAddress {
                        type_: "InternalIP".to_string(),
                        address: "10.0.0.1".to_string(),
                    },
                ]),
                ..Default::default()
            }),
        };

        let node_ip = crate::controller::root::ControllerInner::get_node_ip(&node);
        assert_eq!(node_ip, Some("203.0.113.1".to_string()));
    }

    #[test]
    fn test_get_node_ip_with_internal_ip() {
        let node = Node {
            metadata: Default::default(),
            spec: None,
            status: Some(k8s_openapi::api::core::v1::NodeStatus {
                addresses: Some(vec![k8s_openapi::api::core::v1::NodeAddress {
                    type_: "InternalIP".to_string(),
                    address: "10.0.0.1".to_string(),
                }]),
                ..Default::default()
            }),
        };

        let node_ip = crate::controller::root::ControllerInner::get_node_ip(&node);
        assert_eq!(node_ip, Some("10.0.0.1".to_string()));
    }

    #[test]
    fn test_get_node_ip_no_ip() {
        let node = Node {
            metadata: Default::default(),
            spec: None,
            status: Some(k8s_openapi::api::core::v1::NodeStatus {
                addresses: Some(vec![]),
                ..Default::default()
            }),
        };

        let node_ip = crate::controller::root::ControllerInner::get_node_ip(&node);
        assert_eq!(node_ip, None);
    }

    #[test]
    fn test_get_node_ip_no_status() {
        let node = Node {
            metadata: Default::default(),
            spec: None,
            status: None,
        };

        let node_ip = crate::controller::root::ControllerInner::get_node_ip(&node);
        assert_eq!(node_ip, None);
    }

    #[test]
    fn test_get_node_ip_no_addresses() {
        let node = Node {
            metadata: Default::default(),
            spec: None,
            status: Some(k8s_openapi::api::core::v1::NodeStatus {
                addresses: None,
                ..Default::default()
            }),
        };

        let node_ip = crate::controller::root::ControllerInner::get_node_ip(&node);
        assert_eq!(node_ip, None);
    }

    #[test]
    fn test_ip_command_new() {
        let ip_cmd = IpCommand::new();
        assert!(matches!(ip_cmd, IpCommand));
    }

    #[test]
    fn test_ip_command_run_success() {
        let ip_cmd = IpCommand::new();
        let result = ip_cmd.run(&["link", "show", "lo"]);

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.status.success());
        assert!(!output.stdout.is_empty());
    }

    #[test]
    fn test_ip_command_run_failure() {
        let ip_cmd = IpCommand::new();
        let result = ip_cmd.run(&["nonexistent", "command"]);

        assert!(result.is_err());
    }

    #[test]
    fn test_ip_command_run_with_multiple_args() {
        let ip_cmd = IpCommand::new();
        let result = ip_cmd.run(&["link", "show", "lo"]);

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.status.success());
    }

    #[test]
    fn test_ip_command_run_empty_args() {
        let ip_cmd = IpCommand::new();
        let result = ip_cmd.run(&[]);

        assert!(result.is_err());
    }
}
