## E2E Test Fixes

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