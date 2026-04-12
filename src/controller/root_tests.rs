use k8s_openapi::api::core::v1::Node;
use kube::api::{ListParams, WatchEvent};
use kube_core::watch::{Bookmark, BookmarkMeta};
use std::collections::BTreeMap;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controller::builder::ControllerBuilder;
    use crate::controller::handle::ControllerCommand;

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
        let builder = ControllerBuilder::new();
        let controller = crate::controller::root::Controller::new(builder);
        let handle = controller.handle();

        handle.stop(true).await;

        let _ = controller.await;
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
}
