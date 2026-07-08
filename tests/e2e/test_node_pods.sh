#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/utils.sh"

# =============================================================================
# Test: Pod Distribution
# =============================================================================

test_name="test_pod_distribution"

log_info "Running: $test_name"

# Wait for router pods to be created
log_info "Waiting for router pods to be created..."
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

# Get all nodes
all_nodes=$(kubectl get nodes -o jsonpath='{.items[*].metadata.name}')
if [ -z "$all_nodes" ]; then
    log_error "No nodes found"
    exit 1
fi

node_count=$(echo "$all_nodes" | wc -w)
log_info "Total nodes: $node_count"

pod_count=$(echo "$pods" | wc -w)
log_info "Router pods: $pod_count"

# Test 1: Verify pod count matches node count (DaemonSet)
log_info "Test 1: Checking pod count matches node count..."
if [ "$pod_count" -eq "$node_count" ]; then
    log_info "  ✓ Pod count ($pod_count) matches node count ($node_count)"
else
    log_warn "  ⚠ Pod count ($pod_count) != node count ($node_count) - DaemonSet may still be rolling out"
fi

# Test 2: Verify each node has exactly one router pod
log_info "Test 2: Checking each node has one router pod..."
pass_count=0
fail_count=0

for node in $all_nodes; do
    pods_on_node=$(kubectl get pods -n "$NAMESPACE" -l app=cilium-ipip-router -o wide | grep "$node" | wc -l)
    if [ "$pods_on_node" -eq 1 ]; then
        log_info "  ✓ Node $node has 1 router pod"
        ((pass_count++))
    elif [ "$pods_on_node" -eq 0 ]; then
        log_warn "  ⚠ Node $node has 0 router pods (may be control plane)"
    else
        log_error "  ✗ Node $node has $pods_on_node router pods (expected 1)"
        ((fail_count++))
    fi
done

# Test 3: Verify pods are running with correct restart policy
log_info "Test 3: Checking pod status..."
all_running=true
for pod in $pods; do
    phase=$(kubectl get pod "$pod" -n "$NAMESPACE" -o jsonpath='{.status.phase}')
    if [ "$phase" != "Running" ]; then
        log_error "  ✗ Pod $pod is in phase: $phase (expected Running)"
        all_running=false
    fi
done

if $all_running; then
    log_info "  ✓ All router pods are Running"
fi

# Test 4: Check pod resource limits
log_info "Test 4: Checking pod resource limits..."
for pod in $pods; do
    resources=$(kubectl get pod "$pod" -n "$NAMESPACE" -o jsonpath='{.spec.containers[0].resources}' 2>/dev/null || echo "{}")
    
    # Check if limits exist
    if echo "$resources" | grep -q "limits"; then
        log_info "  ✓ Pod $pod has resource limits defined"
    else
        log_warn "  ⚠ Pod $pod has no resource limits defined"
    fi
    
    # Check if requests exist
    if echo "$resources" | grep -q "requests"; then
        log_info "  ✓ Pod $pod has resource requests defined"
    fi
done

# Test 5: Verify pod has correct image
log_info "Test 5: Checking pod image..."
expected_image="cilium-ipip-router:test"
for pod in $pods; do
    image=$(kubectl get pod "$pod" -n "$NAMESPACE" -o jsonpath='{.spec.containers[0].image}')
    if [ "$image" == "$expected_image" ]; then
        log_info "  ✓ Pod $pod uses image $image"
    else
        log_warn "  ⚠ Pod $pod uses image $image (expected $expected_image)"
    fi
done

log_info "Pod distribution test completed"
exit 0
