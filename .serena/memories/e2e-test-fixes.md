## E2E Test Fixes

### Route Creation for Local Node Issue

**Problem:** E2E tests were failing with routes not being found for any node's pod CIDR.

**Root Cause:** The controller's `update_route_with_executor` function attempted to create routes for ALL nodes being watched, including the local node where the controller pod is running. When processing a watch event for the local node (e.g., node "cilium-router-test-control-plane" with pod CIDR "10.244.0.0/24"), the controller tried to execute:
```
ip route add 10.244.0.0/24 dev tun-...
```
Linux rejected this because the pod CIDR is already directly connected via the pod network, so the `ip route add` command failed and no routes were created.

**Fix:** Added a local hostname detection function that reads the `HOSTNAME` environment variable and compares it against the node name. If they match, the route creation is skipped since the node's own pod CIDR is already accessible without a tunnel route.

### DaemonSet Pod Scheduling Issue

**Problem:** The `cilium-ipip-router` DaemonSet was showing `Desired Number of Nodes Scheduled: 0` - no pods were being created even though the DaemonSet existed.

**Root Cause:** The DaemonSet was being deployed immediately after Cilium installation, but Kubernetes nodes were not yet in `Ready` state. The DaemonSet controller skips NotReady nodes, resulting in 0 desired nodes.

**Fix:** Added a step to wait for all nodes to be Ready before deploying the DaemonSet:

```yaml
- name: Wait for nodes to be ready
  run: |
    kubectl wait --for=condition=Ready nodes --all --timeout=120s || (
      echo 'Nodes not ready after timeout';
      kubectl get nodes -o yaml;
      kubectl get pods -n kube-system -o yaml;
      exit 1;
    )
```

This step is placed after the "Load image into kind cluster" step and before the "Deploy controller" step in `.github/workflows/test.yaml`.

**Commit:** b580c79 - "e2e: wait for nodes to be ready before deploying DaemonSet"

### Route and Tunnel Test Failures

**Problem:** Multiple E2E tests failed with routes not being created or recreated:
- `test_routes`: Routes for pod CIDRs NOT FOUND (only default route visible)
- `test_lifecycle`: Routes not recreated after node uncordon
- `test_tunnels`: Tunnel exists but routes don't exist

**Root Cause:** The controller attempted to add routes for the local node's own pod CIDR, which the kernel rejects because that network is directly connected.

**Fix:** 
- Added `get_local_hostname()` function in `src/ipip/executor.rs` to detect the pod's node via `HOSTNAME` env var
- Added local node skip in `update_route_with_executor()` to skip route creation for the pod's own node
- Exported the function in `src/ipip/mod.rs`
- Added unit test for the new function

### Bash Test Pipefail Fixes

**Problem:** Tests were using `pipefail` but grep pipelines would fail when no matches found, causing script exits with `set -e`.

**Fix:**
- `test_health.sh`: Changed `curl | grep -q "200"` to capture HTTP status in variable, then check with `[ "$http_code" = "200" ]`
- `test_node_pods.sh`: Changed `grep | wc -l` to `grep -c` with `|| true`
- All test files: Added `|| true` to `grep | awk` pipelines

**Commit:** bc1e1c3 - "fix: skip local node routes and fix pipefail issues in tests"