#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/utils.sh"

test_name="test_node_lifecycle"
synthetic_node="ipip-e2e-remote"
synthetic_ip="192.0.2.123"
synthetic_cidr="10.254.0.0/24"
tunnel_name="tun-$(printf '%s' "$synthetic_node" | md5sum | cut -c1-11)"
real_nodes=$(get_all_nodes)

cleanup_node() {
    kubectl delete node "$synthetic_node" --ignore-not-found --wait=false >/dev/null 2>&1 || true
}
trap cleanup_node EXIT

assert_state_on_all_nodes() {
    local expected_state=$1
    local timeout=${2:-60}
    local elapsed=0

    while [ "$elapsed" -lt "$timeout" ]; do
        local matches=0
        local total=0
        local node pod route tunnel tunnel_link

        for node in $real_nodes; do
            pod=$(get_router_pod_for_node "$node")
            [ -z "$pod" ] && continue
            total=$((total + 1))
            route=$(kubectl exec -n "$NAMESPACE" "$pod" -- ip route show "$synthetic_cidr" 2>/dev/null || true)
            tunnel=$(kubectl exec -n "$NAMESPACE" "$pod" -- ip tunnel show "$tunnel_name" 2>/dev/null || true)
            tunnel_link=$(kubectl exec -n "$NAMESPACE" "$pod" -- ip link show dev "$tunnel_name" 2>/dev/null || true)

            if [ "$expected_state" = "present" ]; then
                if [[ "$route" == *"dev $tunnel_name"* && "$tunnel" == *"remote $synthetic_ip"* && -n "$tunnel_link" ]]; then
                    matches=$((matches + 1))
                fi
            elif [ -z "$route" ] && [ -z "$tunnel_link" ]; then
                matches=$((matches + 1))
            fi
        done

        if [ "$total" -gt 0 ] && [ "$matches" -eq "$total" ]; then
            return 0
        fi

        sleep 2
        elapsed=$((elapsed + 2))
    done

    return 1
}

log_info "Running: $test_name"
cleanup_node

kubectl apply -f - <<EOF
apiVersion: v1
kind: Node
metadata:
  name: $synthetic_node
  labels:
    cilium-ipip-router-e2e: synthetic
spec:
  podCIDR: $synthetic_cidr
  podCIDRs:
    - $synthetic_cidr
EOF

kubectl patch node "$synthetic_node" --subresource=status --type=merge \
    -p "{\"status\":{\"addresses\":[{\"type\":\"InternalIP\",\"address\":\"$synthetic_ip\"}]}}" >/dev/null

if assert_state_on_all_nodes present; then
    log_info "  ✓ Added node created a tunnel and route on every router node"
else
    log_error "Added node did not create the expected tunnel and route"
    exit 1
fi

kubectl delete node "$synthetic_node" --wait=true >/dev/null

if assert_state_on_all_nodes absent; then
    log_info "  ✓ Deleted node removed its tunnel and route from every router node"
else
    log_error "Deleted node left a stale tunnel or route"
    exit 1
fi

trap - EXIT
log_info "$test_name PASSED"
