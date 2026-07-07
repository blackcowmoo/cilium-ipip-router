#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/utils.sh"

# =============================================================================
# Test: IPIP Tunnels
# =============================================================================

test_name="test_ipip_tunnels"

log_info "Running: $test_name"

# Get all worker nodes
workers=$(get_worker_nodes)

if [ -z "$workers" ]; then
    log_error "No worker nodes found"
    exit 1
fi

pass_count=0
fail_count=0

for worker in $workers; do
    log_info "Checking tunnels on node: $worker"
    
    # Get worker node IP
    worker_ip=$(kubectl get node "$worker" -o jsonpath='{.status.addresses[?(@.type=="InternalIP")].address}')
    log_info "  Node IP: $worker_ip"
    
    # Get pod CIDR for this node
    pod_cidr=$(kubectl get node "$worker" -o jsonpath='{.spec.podCIDR}')
    log_info "  Pod CIDR: $pod_cidr"
    
    # Get tunnel name (we need to compute it the same way the code does)
    # md5 of node name, first 11 chars of hex, prefixed with "tun-"
    tunnel_name="tun-$(echo -n "$worker" | md5sum | cut -c1-11)"
    log_info "  Expected tunnel name: $tunnel_name"
    
    # Get the pod running on this node (DaemonSet)
    pod_name=$(kubectl get pods -n "$NAMESPACE" -l app=cilium-ipip-router -o wide | grep "$worker" | awk '{print $1}')
    
    if [ -z "$pod_name" ]; then
        log_error "  ✗ No router pod found on node $worker"
        ((fail_count++))
        continue
    fi
    
    log_info "  Checking pod: $pod_name"
    
    # Execute ip tunnel show to verify tunnel exists
    log_info "  Verifying IPIP tunnel exists..."
    if kubectl exec -n "$NAMESPACE" "$pod_name" -- ip tunnel show "$tunnel_name" >/dev/null 2>&1; then
        log_info "  ✓ Tunnel '$tunnel_name' exists"
        
        # Get tunnel details to verify it's IPIP
        tunnel_info=$(kubectl exec -n "$NAMESPACE" "$pod_name" -- ip tunnel show "$tunnel_name")
        
        if echo "$tunnel_info" | grep -q "ipip"; then
            log_info "  ✓ Tunnel is IPIP mode"
        else
            log_error "  ✗ Tunnel is not IPIP mode"
            ((fail_count++))
        fi
        
        if echo "$tunnel_info" | grep -q "local.*$worker_ip" || echo "$tunnel_info" | grep -q "Local: $worker_ip"; then
            log_info "  ✓ Tunnel has correct local IP"
        else
            log_warn "  ⚠ Could not verify local IP in tunnel config"
        fi
        
        ((pass_count++))
    else
        log_error "  ✗ Tunnel '$tunnel_name' NOT FOUND"
        log_error "  Tunnel details: $(kubectl exec -n "$NAMESPACE" "$pod_name" -- ip tunnel list 2>&1 || echo 'command failed')"
        ((fail_count++))
    fi
done

log_info "Results: $pass_count tunnels verified, $fail_count failed"

if [ $fail_count -gt 0 ]; then
    log_error "$test_name FAILED"
    exit 1
fi

log_info "$test_name PASSED"
exit 0
