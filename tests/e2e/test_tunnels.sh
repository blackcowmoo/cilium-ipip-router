#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/utils.sh"

test_name="test_ipip_tunnels"
log_info "Running: $test_name"

nodes=$(get_all_nodes)
node_count=$(wc -w <<<"$nodes")
if [ "$node_count" -lt 2 ]; then
    log_error "At least two nodes are required, found $node_count"
    exit 1
fi

pass_count=0
fail_count=0

wait_for_tunnel() {
    local pod=$1
    local tunnel_name=$2
    local expected_local_ip=$3
    local expected_remote_ip=$4
    local attempt tunnel_info link_info rp_filter

    for ((attempt = 0; attempt < 30; attempt++)); do
        tunnel_info=$(kubectl exec -n "$NAMESPACE" "$pod" -- ip -details tunnel show "$tunnel_name" 2>/dev/null || true)
        link_info=$(kubectl exec -n "$NAMESPACE" "$pod" -- ip -o link show dev "$tunnel_name" 2>/dev/null || true)
        rp_filter=$(kubectl exec -n "$NAMESPACE" "$pod" -- cat "/proc/sys/net/ipv4/conf/$tunnel_name/rp_filter" 2>/dev/null || true)
        if [[ "$tunnel_info" == *"remote $expected_remote_ip"* && \
              "$tunnel_info" == *"local $expected_local_ip"* && \
              "$link_info" == *"UP"* && "$rp_filter" == "0" ]]; then
            return 0
        fi
        sleep 2
    done

    log_error "Last tunnel state: ${tunnel_info:-missing}; link: ${link_info:-missing}; rp_filter: ${rp_filter:-missing}"
    return 1
}

for local_node in $nodes; do
    local_pod=$(get_router_pod_for_node "$local_node")
    local_ip=$(kubectl get node "$local_node" -o jsonpath='{.status.addresses[?(@.type=="InternalIP")].address}')

    if [ -z "$local_pod" ]; then
        log_error "No router pod found on node $local_node"
        fail_count=$((fail_count + 1))
        continue
    fi

    for remote_node in $nodes; do
        tunnel_name="tun-$(printf '%s' "$remote_node" | md5sum | cut -c1-11)"

        if [ "$remote_node" = "$local_node" ]; then
            if kubectl exec -n "$NAMESPACE" "$local_pod" -- ip link show dev "$tunnel_name" >/dev/null 2>&1; then
                log_error "Node $local_node has a tunnel to itself ($tunnel_name)"
                fail_count=$((fail_count + 1))
            fi
            continue
        fi

        remote_ip=$(kubectl get node "$remote_node" -o jsonpath='{.status.addresses[?(@.type=="InternalIP")].address}')
        if wait_for_tunnel "$local_pod" "$tunnel_name" "$local_ip" "$remote_ip"; then
            log_info "  ✓ $local_node -> $remote_node uses UP tunnel $tunnel_name"
            pass_count=$((pass_count + 1))
        else
            log_error "Tunnel $tunnel_name for $remote_node is invalid on $local_node"
            fail_count=$((fail_count + 1))
        fi
    done
done

expected=$((node_count * (node_count - 1)))
log_info "Results: $pass_count/$expected remote tunnels verified, $fail_count failed"

if [ "$fail_count" -gt 0 ] || [ "$pass_count" -ne "$expected" ]; then
    log_error "$test_name FAILED"
    exit 1
fi

cleanup_connectivity_targets() {
    kubectl delete daemonset ipip-connectivity-target -n "$NAMESPACE" --ignore-not-found >/dev/null 2>&1 || true
}
trap cleanup_connectivity_targets EXIT

log_info "Deploying Cilium-managed pods to verify traffic over the IPIP routes..."
kubectl apply -f - <<EOF
apiVersion: apps/v1
kind: DaemonSet
metadata:
  name: ipip-connectivity-target
  namespace: $NAMESPACE
spec:
  selector:
    matchLabels:
      app: ipip-connectivity-target
  template:
    metadata:
      labels:
        app: ipip-connectivity-target
    spec:
      tolerations:
        - key: node-role.kubernetes.io/control-plane
          effect: NoSchedule
      containers:
        - name: http
          image: nginx:1.29.1-alpine
          ports:
            - containerPort: 80
EOF
kubectl rollout status daemonset/ipip-connectivity-target -n "$NAMESPACE" --timeout=180s

connectivity_passed=0
connectivity_failed=0
for local_node in $nodes; do
    local_pod=$(get_router_pod_for_node "$local_node")
    for remote_node in $nodes; do
        [ "$remote_node" = "$local_node" ] && continue

        remote_pod_ip=$(kubectl get pods -n "$NAMESPACE" -l app=ipip-connectivity-target \
            --field-selector "spec.nodeName=$remote_node" -o jsonpath='{.items[0].status.podIP}')
        http_code=$(kubectl exec -n "$NAMESPACE" "$local_pod" -- \
            curl --silent --output /dev/null --write-out '%{http_code}' \
            --connect-timeout 5 "http://$remote_pod_ip" 2>/dev/null || true)

        if [ "$http_code" = "200" ]; then
            log_info "  ✓ $local_node reached pod $remote_pod_ip on $remote_node"
            connectivity_passed=$((connectivity_passed + 1))
        else
            log_error "  ✗ $local_node could not reach pod $remote_pod_ip on $remote_node (HTTP ${http_code:-none})"
            connectivity_failed=$((connectivity_failed + 1))
        fi
    done
done

cleanup_connectivity_targets
trap - EXIT

if [ "$connectivity_failed" -gt 0 ] || [ "$connectivity_passed" -ne "$expected" ]; then
    log_error "$test_name FAILED: traffic did not traverse every remote-node route"
    exit 1
fi

log_info "$test_name PASSED"
