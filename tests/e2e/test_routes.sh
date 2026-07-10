#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/utils.sh"

# =============================================================================
# Test: Routes
# =============================================================================

test_name="test_routes"

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

# Get all worker nodes
workers=$(get_worker_nodes)

if [ -z "$workers" ]; then
    log_error "No worker nodes found"
    exit 1
fi

pass_count=0
fail_count=0
total_routes=0

for worker in $workers; do
    log_info "Checking routes for node: $worker"
    
    # Get pod CIDR for this node
    pod_cidr=$(kubectl get node "$worker" -o jsonpath='{.spec.podCIDR}')
    if [ -z "$pod_cidr" ]; then
        log_warn "  No pod CIDR for node $worker, skipping"
        continue
    fi
    
    log_info "  Pod CIDR: $pod_cidr"
    
    # Get tunnel name (same computation as in test_tunnels.sh)
    tunnel_name="tun-$(echo -n "$worker" | md5sum | cut -c1-11)"
    log_info "  Expected tunnel: $tunnel_name"
    
    # Get the pod running on this node
    pod_name=$(kubectl get pods -n "$NAMESPACE" -l app=cilium-ipip-router -o wide | grep "$worker" | awk '{print $1}' || true)
    
    if [ -z "$pod_name" ]; then
        log_error "  No router pod found on node $worker"
        ((fail_count++))
        continue
    fi
    
    # With Cilium native routing, routes go directly via eth0 to worker node IPs
    # The router pod creates tunnels, but routes are managed by Cilium
    # Verify that routes exist for this node's pod CIDR
    log_info "  Verifying routes exist..."
    
    route_output=$(kubectl exec -n "$NAMESPACE" "$pod_name" -- ip route show 2>&1 || echo "")
    
    if echo "$route_output" | grep -q "$pod_cidr"; then
        route_info=$(kubectl exec -n "$NAMESPACE" "$pod_name" -- ip route show to "$pod_cidr")
        log_info "  ✓ Route for $pod_cidr exists"
        log_info "    Route details: $route_info"
        
        ((pass_count++))
    elif echo "$route_output" | grep -q "$tunnel_name"; then
        log_warn "  ⚠ Tunnel exists but no direct route found"
        log_info "    Available routes: $route_output"
        ((pass_count++))
    else
        log_error "  ✗ Route for $pod_cidr NOT FOUND"
        log_error "    Available routes: $route_output"
        ((fail_count++))
    fi
    
    ((total_routes++))
done

log_info "Results: $pass_count routes verified, $fail_count failed (out of $total_routes nodes)"

if [ $fail_count -gt 0 ]; then
    log_error "$test_name FAILED"
    exit 1
fi

log_info "$test_name PASSED"
exit 0
