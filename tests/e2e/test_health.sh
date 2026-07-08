#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/utils.sh"

# =============================================================================
# Test: Health Endpoint
# =============================================================================

test_name="test_health_endpoint"

log_info "Running: $test_name"

# Wait for router pods to be created
log_info "Waiting for router pods to be created..."
pods=""
wait_count=0
while [ $wait_count -lt 180 ]; do
    pods=$(kubectl get pods -n "$NAMESPACE" -l app=cilium-ipip-router -o jsonpath='{.items[*].metadata.name}' 2>/dev/null || echo "")
    if [ -n "$pods" ]; then
        log_info "  Found pod(s): $pods"
        break
    fi
    sleep 5
    wait_count=$((wait_count + 5))
done

if [ -z "$pods" ]; then
    log_error "No router pods found after waiting"
    log_info "Debugging: DaemonSet details..."
    kubectl describe daemonset cilium-ipip-router -n "$NAMESPACE" || true
    log_info "Debugging: All pods in $NAMESPACE namespace..."
    kubectl get pods -n "$NAMESPACE" -o wide || true
    log_info "Debugging: Pod events..."
    kubectl get events -n "$NAMESPACE" --sort-by='.lastTimestamp' 2>/dev/null | tail -20 || true
    exit 1
fi

pass_count=0
fail_count=0

for pod in $pods; do
    log_info "Checking health endpoint for pod: $pod"
    
    # Test readiness probe endpoint
    if kubectl exec -n "$NAMESPACE" "$pod" -- curl -s -o /dev/null -w "%{http_code}" http://localhost:9090/health | grep -q "200"; then
        log_info "  ✓ Health endpoint returned 200 OK"
        ((pass_count++))
    else
        log_error "  ✗ Health endpoint failed for pod $pod"
        ((fail_count++))
    fi
    
    # Test liveness probe endpoint
    if kubectl exec -n "$NAMESPACE" "$pod" -- curl -s -o /dev/null -w "%{http_code}" http://localhost:9090/health | grep -q "200"; then
        log_info "  ✓ Liveness endpoint returned 200 OK"
    else
        log_error "  ✗ Liveness endpoint failed for pod $pod"
        ((fail_count++))
    fi
done

log_info "Results: $pass_count passed, $fail_count failed"

if [ $fail_count -gt 0 ]; then
    log_error "$test_name FAILED"
    exit 1
fi

log_info "$test_name PASSED"
exit 0
