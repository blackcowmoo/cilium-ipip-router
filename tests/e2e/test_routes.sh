#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/utils.sh"

# =============================================================================
# Test: Routes
# =============================================================================

test_name="test_routes"

log_info "Running: $test_name"

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
    pod_name=$(kubectl get pods -n "$NAMESPACE" -l app=cilium-ipip-router -o wide | grep "$worker" | awk '{print $1}')
    
    if [ -z "$pod_name" ]; then
        log_error "  No router pod found on node $worker"
        ((fail_count++))
        continue
    fi
    
    # Check if route exists pointing to this tunnel
    log_info "  Verifying route via '$tunnel_name'..."
    
    if kubectl exec -n "$NAMESPACE" "$pod_name" -- ip route show to "$pod_cidr" | grep -q "$tunnel_name"; then
        route_info=$(kubectl exec -n "$NAMESPACE" "$pod_name" -- ip route show to "$pod_cidr")
        log_info "  ✓ Route for $pod_cidr via $tunnel_name exists"
        log_info "    Route details: $route_info"
        
        # Count total routes
        route_count=$(kubectl exec -n "$NAMESPACE" "$pod_name" -- ip route show | grep -c "tun-")
        log_info "  Total routes via tunnels on this node: $route_count"
        
        ((pass_count++))
    else
        log_error "  ✗ Route for $pod_cidr via $tunnel_name NOT FOUND"
        log_error "    Available routes: $(kubectl exec -n "$NAMESPACE" "$pod_name" -- ip route show 2>&1 || echo 'command failed')"
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
