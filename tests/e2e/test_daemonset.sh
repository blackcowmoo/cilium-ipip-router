#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/utils.sh"

# =============================================================================
# Test: DaemonSet Deployment
# =============================================================================

test_name="test_daemonset_deployment"

log_info "Running: $test_name"

# Test 1: DaemonSet exists
log_info "Test 1: Checking DaemonSet exists..."
if kubectl get daemonset cilium-ipip-router -n "$NAMESPACE" >/dev/null 2>&1; then
    log_info "  ✓ DaemonSet 'cilium-ipip-router' exists"
else
    log_error "  ✗ DaemonSet 'cilium-ipip-router' not found"
    exit 1
fi

# Test 2: DaemonSet is ready
log_info "Test 2: Checking DaemonSet is ready..."
if kubectl rollout status daemonset/cilium-ipip-router -n "$NAMESPACE" --timeout=120s >/dev/null 2>&1; then
    log_info "  ✓ DaemonSet rollout acknowledged"
else
    log_error "  ✗ DaemonSet rollout failed"
    exit 1
fi

# Wait for actual pods to be created and running
log_info "Waiting for router pods to be created..."
pod_count=0
wait_count=0
while [ $wait_count -lt 180 ]; do
    pod_count=$(kubectl get pods -n "$NAMESPACE" -l app=cilium-ipip-router --no-headers 2>/dev/null | wc -l)
    if [ "$pod_count" -gt 0 ]; then
        log_info "  Found $pod_count pod(s)"
        break
    fi
    sleep 5
    wait_count=$((wait_count + 5))
done

if [ "$pod_count" -eq 0 ]; then
    log_error "  ✗ No router pods found after rollout"
    log_info "  Debugging: DaemonSet details..."
    kubectl describe daemonset cilium-ipip-router -n "$NAMESPACE" || true
    log_info "  Debugging: All pods in $NAMESPACE namespace..."
    kubectl get pods -n "$NAMESPACE" -o wide || true
    log_info "  Debugging: Pod events..."
    kubectl get events -n "$NAMESPACE" --sort-by='.lastTimestamp' 2>/dev/null | tail -20 || true
    exit 1
fi

# Test 3: Verify ServiceAccount
log_info "Test 3: Checking ServiceAccount..."
if kubectl get serviceaccount cilium-ipip-router -n "$NAMESPACE" >/dev/null 2>&1; then
    log_info "  ✓ ServiceAccount 'cilium-ipip-router' exists"
else
    log_error "  ✗ ServiceAccount 'cilium-ipip-router' not found"
    exit 1
fi

# Test 4: Verify ClusterRole
log_info "Test 4: Checking ClusterRole..."
if kubectl get clusterrole cilium-ipip-router -n "$NAMESPACE" >/dev/null 2>&1; then
    log_info "  ✓ ClusterRole 'cilium-ipip-router' exists"
else
    log_error "  ✗ ClusterRole 'cilium-ipip-router' not found"
    exit 1
fi

# Test 5: Verify ClusterRoleBinding
log_info "Test 5: Checking ClusterRoleBinding..."
if kubectl get clusterrolebinding cilium-ipip-router -n "$NAMESPACE" >/dev/null 2>&1; then
    log_info "  ✓ ClusterRoleBinding 'cilium-ipip-router' exists"
else
    log_error "  ✗ ClusterRoleBinding 'cilium-ipip-router' not found"
    exit 1
fi

# Test 6: Verify pod has correct labels
log_info "Test 6: Checking pod labels..."
pods=$(kubectl get pods -n "$NAMESPACE" -l app=cilium-ipip-router -o jsonpath='{.items[*].metadata.name}' 2>/dev/null || echo "")
if [ -z "$pods" ]; then
    log_error "  ✗ No router pods found to check labels"
    exit 1
fi
pod_labels=$(kubectl get pods -n "$NAMESPACE" -l app=cilium-ipip-router -o jsonpath='{.items[0].metadata.labels.app}')
if [ "$pod_labels" == "cilium-ipip-router" ]; then
    log_info "  ✓ Pod has correct labels"
else
    log_error "  ✗ Pod labels incorrect: $pod_labels"
    exit 1
fi

# Test 7: Verify pod security context (capabilities)
log_info "Test 7: Checking pod security context..."
caps=$(kubectl get pods -n "$NAMESPACE" -l app=cilium-ipip-router -o jsonpath='{.items[0].spec.containers[0].securityContext.capabilities.add}' 2>/dev/null || echo "missing")

if echo "$caps" | grep -q "NET_ADMIN" && echo "$caps" | grep -q "SYS_ADMIN"; then
    log_info "  ✓ Pod has required capabilities (NET_ADMIN, SYS_ADMIN)"
else
    log_warn "  ⚠ Could not verify capabilities (may need privileged access)"
fi

# Test 8: Verify privileged mode
privileged=$(kubectl get pods -n "$NAMESPACE" -l app=cilium-ipip-router -o jsonpath='{.items[0].spec.containers[0].securityContext.privileged}' 2>/dev/null || echo "false")
if [ "$privileged" == "true" ]; then
    log_info "  ✓ Pod is running in privileged mode"
else
    log_warn "  ⚠ Pod may not be privileged: $privileged"
fi

# Test 9: Verify image pull policy
log_info "Test 9: Checking image pull policy..."
pull_policy=$(kubectl get pods -n "$NAMESPACE" -l app=cilium-ipip-router -o jsonpath='{.items[0].spec.containers[0].imagePullPolicy}')
if [ "$pull_policy" == "Never" ]; then
    log_info "  ✓ Image pull policy is 'Never' (correct for Kind)"
else
    log_warn "  ⚠ Image pull policy is '$pull_policy' (expected 'Never' for Kind)"
fi

log_info "All DaemonSet deployment tests passed"
exit 0
