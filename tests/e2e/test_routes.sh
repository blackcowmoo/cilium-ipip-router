#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/utils.sh"

test_name="test_ipip_routes"
log_info "Running: $test_name"

nodes=$(get_all_nodes)
node_count=$(wc -w <<<"$nodes")
if [ "$node_count" -lt 2 ]; then
    log_error "At least two nodes are required, found $node_count"
    exit 1
fi

pass_count=0
fail_count=0

wait_for_route() {
    local pod=$1
    local cidr=$2
    local tunnel_name=$3
    local attempt route

    for ((attempt = 0; attempt < 30; attempt++)); do
        route=$(kubectl exec -n "$NAMESPACE" "$pod" -- ip route show "$cidr" 2>/dev/null || true)
        if [[ "$route" == *"dev $tunnel_name"* ]]; then
            return 0
        fi
        sleep 2
    done

    log_error "Last route state for $cidr: ${route:-missing}"
    return 1
}

for local_node in $nodes; do
    local_pod=$(get_router_pod_for_node "$local_node")
    if [ -z "$local_pod" ]; then
        log_error "No router pod found on node $local_node"
        fail_count=$((fail_count + 1))
        continue
    fi

    for remote_node in $nodes; do
        [ "$remote_node" = "$local_node" ] && continue

        remote_cidr=$(kubectl get node "$remote_node" -o jsonpath='{.spec.podCIDR}')
        tunnel_name="tun-$(printf '%s' "$remote_node" | md5sum | cut -c1-11)"
        if [ -z "$remote_cidr" ]; then
            log_error "Node $remote_node has no PodCIDR"
            fail_count=$((fail_count + 1))
        elif wait_for_route "$local_pod" "$remote_cidr" "$tunnel_name"; then
            log_info "  ✓ $local_node routes $remote_cidr through $tunnel_name"
            pass_count=$((pass_count + 1))
        else
            log_error "Route for $remote_node ($remote_cidr) is incorrect on $local_node"
            fail_count=$((fail_count + 1))
        fi
    done
done

expected=$((node_count * (node_count - 1)))
log_info "Results: $pass_count/$expected remote routes verified, $fail_count failed"

if [ "$fail_count" -gt 0 ] || [ "$pass_count" -ne "$expected" ]; then
    log_error "$test_name FAILED"
    exit 1
fi

log_info "$test_name PASSED"
