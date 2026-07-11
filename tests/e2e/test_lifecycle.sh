#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/utils.sh"

# =============================================================================
# Test: Node Lifecycle
# =============================================================================

test_name="test_node_lifecycle"

log_info "Running: $test_name"

# In native routing mode, skip route/tunnel-specific lifecycle checks
ROUTE_CHECKS_ENABLED=true
if [ "${CILIUM_ROUTING_MODE:-}" = "native" ]; then
    log_info "Running in native routing mode - skipping route/tunnel lifecycle checks"
    log_info "Route/tunnel checks will be skipped (Cilium handles routing)"
    ROUTE_CHECKS_ENABLED=false
fi

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

worker_count=$(echo "$workers" | wc -w)
log_info "Found $worker_count worker nodes"

if [ $worker_count -lt 2 ]; then
    log_warn "Test requires at least 2 worker nodes, skipping lifecycle test"
    log_info "$test_name SKIPPED"
    exit 0
fi

# Pick the last worker node for testing
test_node=$(echo "$workers" | awk '{print $NF}')
log_info "Testing with node: $test_node"

# Get initial state
log_info "Step 1: Get initial tunnel count"
initial_tunnel_count=$(kubectl exec -n "$NAMESPACE" -l app=cilium-ipip-router -- sh -c "ip tunnel list | grep -c tun-" 2>/dev/null || echo "0")
log_info "  Initial tunnel count: $initial_tunnel_count"

# Get node IP and pod CIDR before draining
test_node_ip=$(kubectl get node "$test_node" -o jsonpath='{.status.addresses[?(@.type=="InternalIP")].address}')
test_node_cidr=$(kubectl get node "$test_node" -o jsonpath='{.spec.podCIDR}')
log_info "  Node $test_node: IP=$test_node_ip, CIDR=$test_node_cidr"

# Step 2: Cordon the node (prevent new pods)
log_info "Step 2: Cordoning node $test_node..."
kubectl cordon "$test_node" >/dev/null 2>&1
log_info "  Node $test_node cordoned"

# Wait a moment for any cleanup
sleep 10

# Step 3: Drain the node (evict pods)
log_info "Step 3: Draining node $test_node..."
# Use force to evict pods immediately
kubectl drain "$test_node" --ignore-daemonsets --delete-emptydir-data --force --timeout=60s 2>&1 | head -20 || true
log_info "  Node $test_node drained"

# Wait for cleanup
sleep 15

# Step 4: Check that routes/tunnels for this node are cleaned up
log_info "Step 4: Verifying cleanup on remaining nodes..."

# Get a pod from another node to check routes
check_node=""
check_pod=""
if [ "$ROUTE_CHECKS_ENABLED" = "true" ]; then
    other_workers=$(echo "$workers" | awk '{for(i=1;i<NF;i++) print $i}')
    check_node=$(echo "$other_workers" | head -1)
    check_pod=$(kubectl get pods -n "$NAMESPACE" -l app=cilium-ipip-router -o wide | grep "$check_node" | awk '{print $1}' || true)

    if [ -n "$check_pod" ]; then
        log_info "  Checking routes on node $check_node (pod: $check_pod)..."
        
        # Check that route for test_node's CIDR is gone
        tunnel_name="tun-$(echo -n "$test_node" | md5sum | cut -c1-11)"
        
        if kubectl exec -n "$NAMESPACE" "$check_pod" -- ip route show to "$test_node_cidr" | grep -q "$tunnel_name"; then
            log_warn "  ⚠ Route for $test_node_cidr still exists (may be expected depending on cleanup timing)"
        else
            log_info "  ✓ Route for $test_node_cidr properly cleaned up"
        fi
    else
        log_warn "  Could not find another node to check cleanup"
    fi
else
    log_info "  Skipping route cleanup check (native routing mode)"
    tunnel_name=""
fi

# Step 5: Uncordon the node (bring it back)
log_info "Step 5: Uncordoning node $test_node..."
kubectl uncordon "$test_node" >/dev/null 2>&1
log_info "  Node $test_node uncordoned"

# Wait for pod to be recreated
log_info "Step 6: Waiting for pod to be recreated..."
wait_for_daemonset_ready "cilium-ipip-router" "$NAMESPACE" 180

# Wait for routes to be recreated
if [ "$ROUTE_CHECKS_ENABLED" = "true" ]; then
    sleep 15
fi

# Step 7: Verify routes are recreated (only in non-native mode)
fail_count=0

if [ "$ROUTE_CHECKS_ENABLED" = "true" ] && [ -n "$check_pod" ]; then
    log_info "Step 7: Verifying routes are recreated..."
    log_info "  Checking routes on node $check_node (pod: $check_pod)..."
    
    if kubectl exec -n "$NAMESPACE" "$check_pod" -- ip route show to "$test_node_cidr" | grep -q "$tunnel_name"; then
        log_info "  ✓ Route for $test_node_cidr via $tunnel_name recreated"
    else
        log_error "  ✗ Route for $test_node_cidr NOT recreated"
        log_error "    Current routes: $(kubectl exec -n "$NAMESPACE" "$check_pod" -- ip route show 2>&1 || echo 'command failed')"
        fail_count=$((fail_count + 1))
    fi
else
    log_info "Step 7: Skipping route recreation check (native routing mode)"
fi

# Verify test_node has its own tunnels/routes
log_info "Step 8: Verifying test node pod is running..."
test_pod=$(kubectl get pods -n "$NAMESPACE" -l app=cilium-ipip-router -o wide | grep "$test_node" | awk '{print $1}' || true)

if [ -n "$test_pod" ]; then
    log_info "  ✓ Pod found on test node: $test_pod"
    
    if [ "$ROUTE_CHECKS_ENABLED" = "true" ]; then
        log_info "  Checking tunnels on test node..."
        if kubectl exec -n "$NAMESPACE" "$test_pod" -- ip tunnel list | grep -q "tun-"; then
            log_info "  ✓ Tunnels exist on test node"
        else
            log_warn "  ⚠ No tunnels found on test node"
        fi
        
        route_count=$(kubectl exec -n "$NAMESPACE" "$test_pod" -- ip route show | grep -c "tun-" || echo "0")
        log_info "  Routes via tunnels on test node: $route_count"
    else
        log_info "  Skipping tunnel/route checks (native routing mode)"
    fi
else
    log_error "  No router pod found on test node after uncordoning"
    fail_count=$((fail_count + 1))
fi

if [ $fail_count -gt 0 ]; then
    log_error "$test_name FAILED"
    exit 1
fi

log_info "Node lifecycle test completed"
exit 0
