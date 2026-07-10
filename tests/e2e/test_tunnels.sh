#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/utils.sh"

# =============================================================================
# Test: Tunnel Configuration & Connectivity
# =============================================================================

test_name="test_ipip_tunnels"

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
    
    # Get tunnel name (same computation as in the router binary)
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
    
    # Verify tunnel interface exists (router process creates it)
    log_info "  Verifying tunnel interface exists..."
    if kubectl exec -n "$NAMESPACE" "$pod_name" -- ip tunnel show "$tunnel_name" >/dev/null 2>&1; then
        log_info "  ✓ Tunnel '$tunnel_name' exists"
        
        # Get tunnel details to verify configuration
        tunnel_info=$(kubectl exec -n "$NAMESPACE" "$pod_name" -- ip tunnel show "$tunnel_name" 2>&1)
        
        # Verify local IP matches worker IP
        if echo "$tunnel_info" | grep -q "local $worker_ip"; then
            log_info "  ✓ Tunnel has correct local IP: $worker_ip"
        else
            log_warn "  ⚠ Local IP in tunnel config may differ (expected $worker_ip)"
        fi
        
        ((pass_count++))
    else
        log_error "  ✗ Tunnel '$tunnel_name' NOT FOUND"
        ((fail_count++))
    fi
done

# Test connectivity between pods on different worker nodes
if [ $(echo "$workers" | wc -w) -ge 2 ]; then
    log_info "Testing inter-node connectivity between router pods..."
    
    worker_list=($workers)
    node_a="${worker_list[0]}"
    node_b="${worker_list[1]}"
    
    pod_a=$(kubectl get pods -n "$NAMESPACE" -l app=cilium-ipip-router -o wide | grep "$node_a" | awk '{print $1}')
    pod_b=$(kubectl get pods -n "$NAMESPACE" -l app=cilium-ipip-router -o wide | grep "$node_b" | awk '{print $1}')
    
    if [ -n "$pod_a" ] && [ -n "$pod_b" ]; then
        # Get pod B's IP
        pod_b_ip=$(kubectl get pod "$pod_b" -n "$NAMESPACE" -o jsonpath='{.status.podIP}')
        log_info "  Pod A: $pod_a ($node_a), Pod B: $pod_b ($node_b), Pod B IP: $pod_b_ip"
        
        # Try to curl pod B's health endpoint from pod A
        if kubectl exec -n "$NAMESPACE" "$pod_a" -- curl -s -o /dev/null -w "%{http_code}" --connect-timeout 5 "http://$pod_b_ip:9090/health" 2>/dev/null | grep -q "200"; then
            log_info "  ✓ Pod A can reach Pod B's health endpoint"
            ((pass_count++))
        else
            log_error "  ✗ Pod A cannot reach Pod B's health endpoint"
            ((fail_count++))
        fi
        
        # Reverse: try to curl pod A's health endpoint from pod B
        pod_a_ip=$(kubectl get pod "$pod_a" -n "$NAMESPACE" -o jsonpath='{.status.podIP}')
        if kubectl exec -n "$NAMESPACE" "$pod_b" -- curl -s -o /dev/null -w "%{http_code}" --connect-timeout 5 "http://$pod_a_ip:9090/health" 2>/dev/null | grep -q "200"; then
            log_info "  ✓ Pod B can reach Pod A's health endpoint"
            ((pass_count++))
        else
            log_error "  ✗ Pod B cannot reach Pod A's health endpoint"
            ((fail_count++))
        fi
    fi
fi

log_info "Results: $pass_count checks passed, $fail_count failed"

if [ $fail_count -gt 0 ]; then
    log_error "$test_name FAILED"
    exit 1
fi

log_info "$test_name PASSED"
exit 0
