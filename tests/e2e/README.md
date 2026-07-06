# E2E Tests

This directory contains end-to-end tests for the Cilium IPIP Router using Kind.

## Running Tests

### Prerequisites

1. Install Kind: https://kind.sigs.k8s.io/docs/user/quick-start/
2. Install kubectl: https://kubernetes.io/docs/tasks/tools/install-kubectl/
3. Build the Docker image:
   ```bash
   docker build -t cilium-ipip-router:test .
   ```

### Run All E2E Tests

```bash
cd /git/work
bash tests/e2e/run.sh
```

Or specify a cluster name:

```bash
bash tests/e2e/run.sh my-cluster
```

### Run Individual Tests

```bash
bash tests/e2e/test_health.sh
bash tests/e2e/test_daemonset.sh
bash tests/e2e/test_tunnels.sh
bash tests/e2e/test_routes.sh
bash tests/e2e/test_node_pods.sh
bash tests/e2e/test_lifecycle.sh
```

## Test Coverage

The E2E test suite covers:

1. **Health Endpoint** (`test_health.sh`)
   - Verifies `/health` endpoint returns 200 OK on all router pods

2. **DaemonSet Deployment** (`test_daemonset.sh`)
   - Validates DaemonSet exists and is fully rolled out
   - Verifies RBAC resources (ServiceAccount, ClusterRole, ClusterRoleBinding)
   - Checks pod labels and security context

3. **IPIP Tunnels** (`test_tunnels.sh`)
   - Verifies IPIP tunnels exist on each worker node
   - Validates tunnel configuration (mode, local/remote IPs)

4. **Routes** (`test_routes.sh`)
   - Verifies kernel routes exist for each node's pod CIDR
   - Validates routes point to correct tunnels

5. **Pod Distribution** (`test_node_pods.sh`)
   - Verifies exactly one router pod per node (DaemonSet behavior)
   - Checks pod status and resource limits

6. **Node Lifecycle** (`test_lifecycle.sh`)
   - Tests node cordon/drain/uncordon workflow
   - Verifies tunnel and route cleanup/creation
   - Tests pod restart and recovery

7. **Rust-based E2E Tests** (`e2e_integration_tests.rs`)
   - Controller watch functionality
   - Node API interaction
   - Health check via Kubernetes API

## CI Integration

The E2E tests are run as part of the GitHub Actions workflow in `.github/workflows/test.yaml` under the `e2e-tests` job.

## Adding New Tests

1. Create a new test script in `tests/e2e/test_<name>.sh`
2. Use `source "$SCRIPT_DIR/utils.sh"` for common utilities
3. Register the test in `tests/e2e/run.sh`
4. Add corresponding Rust tests in `tests/e2e/e2e_integration_tests.rs` if applicable

## Debugging

When tests fail, logs are collected to `./e2e-logs/`:
- Router pod logs from each node
- Kind cluster logs (via `kind export logs`)

Review these logs to understand failures.
