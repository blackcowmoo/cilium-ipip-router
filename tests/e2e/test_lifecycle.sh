#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/utils.sh"

# All three reads must succeed. An exec/API failure is not evidence of deletion.
node_has_expected_state() {
    local pod=$1 expected_state=$2
    local routes tunnels links
    routes=$(kubectl exec -n "$NAMESPACE" "$pod" -- ip route show) || return 1
    tunnels=$(kubectl exec -n "$NAMESPACE" "$pod" -- ip tunnel show) || return 1
    links=$(kubectl exec -n "$NAMESPACE" "$pod" -- ip -o link show) || return 1

    local route_count tunnel_count link_count
    route_count=$(awk -v cidr="$synthetic_cidr" '$1 == cidr {n++} END {print n+0}' <<<"$routes")
    tunnel_count=$(awk -v name="$tunnel_name:" '$1 == name {n++} END {print n+0}' <<<"$tunnels")
    link_count=$(awk -v name="$tunnel_name" '{split($2, a, "[@:]"); if (a[1] == name) n++} END {print n+0}' <<<"$links")

    case "$expected_state" in
        present)
            [ "$route_count" -eq 1 ] && [ "$tunnel_count" -eq 1 ] && [ "$link_count" -eq 1 ] || return 1
            awk -v cidr="$synthetic_cidr" -v dev="$tunnel_name" '
                $1 == cidr {for (i=2; i<NF; i++) if ($i == "dev" && $(i+1) == dev) ok=1}
                END {exit !ok}' <<<"$routes" || return 1
            awk -v name="$tunnel_name:" -v ip="$synthetic_ip" '
                $1 == name && $2 == "ip/ip" {for (i=3; i<NF; i++) if ($i == "remote" && $(i+1) == ip) ok=1}
                END {exit !ok}' <<<"$tunnels" || return 1
            awk -v name="$tunnel_name" '
                {split($2, a, "[@:]"); if (a[1] == name && $3 ~ /[<,]UP[,>]/) ok=1}
                END {exit !ok}' <<<"$links"
            ;;
        absent)
            [ "$route_count" -eq 0 ] && [ "$tunnel_count" -eq 0 ] && [ "$link_count" -eq 0 ]
            ;;
        *) return 1 ;;
    esac
}

state_matches_all_nodes() {
    local expected_state=$1 node pod
    [ -n "$real_nodes" ] || return 1
    for node in $real_nodes; do
        pod=$(get_router_pod_for_node "$node") || return 1
        [ -n "$pod" ] || return 1
        node_has_expected_state "$pod" "$expected_state" || return 1
    done
}

assert_state_on_all_nodes() {
    local expected_state=$1 timeout=${2:-60} elapsed=0
    while [ "$elapsed" -lt "$timeout" ]; do
        if state_matches_all_nodes "$expected_state"; then
            return 0
        fi
        sleep 2
        elapsed=$((elapsed + 2))
    done
    log_error "Timed out waiting for $expected_state state on every router node"
    return 1
}

create_synthetic_node() {
    kubectl create -f - <<EOF_NODE
apiVersion: v1
kind: Node
metadata:
  name: $synthetic_node
  labels:
    cilium-ipip-router-e2e: synthetic
spec:
  unschedulable: true
  podCIDR: $synthetic_cidr
  podCIDRs:
    - $synthetic_cidr
EOF_NODE
    owns_node=true
    kubectl patch node "$synthetic_node" --subresource=status --type=merge \
        -p "{\"status\":{\"addresses\":[{\"type\":\"InternalIP\",\"address\":\"$synthetic_ip\"}]}}" >/dev/null
}

cleanup_lifecycle() {
    local exit_code=$?
    trap - EXIT
    if [ "$exit_code" -ne 0 ]; then
        collect_logs || true
    fi
    if [ "$owns_node" = true ]; then
        kubectl delete node "$synthetic_node" --ignore-not-found --wait=true >/dev/null || exit_code=1
        assert_state_on_all_nodes absent || exit_code=1
    fi
    exit "$exit_code"
}

main() {
    # A unique name plus create (not apply) avoids modifying another run's Node.
    synthetic_node="ipip-e2e-$(cat /proc/sys/kernel/random/uuid)"
    synthetic_ip="192.0.2.123"
    synthetic_cidr="10.254.0.0/24"
    tunnel_name="tun-$(printf '%s' "$synthetic_node" | md5sum | cut -c1-11)"
    real_nodes=$(get_all_nodes)
    owns_node=false
    trap cleanup_lifecycle EXIT

    if [ "$(wc -w <<<"$real_nodes")" -lt 2 ]; then
        log_error "Lifecycle tests require at least two real nodes"
        return 1
    fi
    # Refuse a CIDR already allocated by the cluster.
    local allocated_cidrs
    allocated_cidrs=$(kubectl get nodes -o jsonpath='{.items[*].spec.podCIDRs[*]}')
    if tr ' ' '\n' <<<"$allocated_cidrs" | grep -Fxq "$synthetic_cidr"; then
        log_error "Synthetic PodCIDR is already allocated"
        return 1
    fi

    log_info "Running: test_node_lifecycle"
    assert_state_on_all_nodes absent
    create_synthetic_node
    assert_state_on_all_nodes present
    log_info "  ✓ Added node created an UP tunnel and exact route on every node"

    kubectl delete node "$synthetic_node" --wait=true >/dev/null
    assert_state_on_all_nodes absent
    owns_node=false
    log_info "  ✓ Deleted node removed its tunnel and route from every node"

    # Reuse the identity with a different endpoint to expose stale tunnel state.
    synthetic_ip="192.0.2.124"
    create_synthetic_node
    assert_state_on_all_nodes present
    log_info "  ✓ Recreated node reconciled the new remote endpoint on every node"

    kubectl delete node "$synthetic_node" --wait=true >/dev/null
    assert_state_on_all_nodes absent
    owns_node=false
    log_info "test_node_lifecycle PASSED"
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
    main "$@"
fi
