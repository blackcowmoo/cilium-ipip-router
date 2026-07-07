#!/usr/bin/env bash
set -euo pipefail

# =============================================================================
# E2E Test Utilities
# =============================================================================

E2E_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$E2E_DIR/.." && pwd)"

NAMESPACE="${NAMESPACE:-default}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Wait for a pod to be ready
wait_for_pod_ready() {
    local namespace=$1
    local label=$2
    local timeout=${3:-300}

    log_info "Waiting for pods with label '$label' to be ready..."
    
    local count=0
    while [ $count -lt $timeout ]; do
        local ready=$(kubectl get pods -n "$namespace" -l "$label" -o jsonpath='{.items[*].status.containerStatuses[*].ready}' 2>/dev/null | tr ' ' '\n' | grep -c "true" || echo "0")
        local total=$(kubectl get pods -n "$namespace" -l "$label" -o jsonpath='{.items[*].status.containerStatuses[*].name}' 2>/dev/null | wc -w || echo "0")
        
        if [ "$ready" -eq "$total" ] && [ "$total" -gt 0 ]; then
            log_info "All pods are ready!"
            return 0
        fi
        
        sleep 5
        count=$((count + 5))
    done
    
    log_error "Pods did not become ready within ${timeout}s"
    return 1
}

# Wait for DaemonSet to be fully rolled out
wait_for_daemonset_ready() {
    local name=$1
    local namespace=${2:-default}
    local timeout=${3:-300}

    log_info "Waiting for DaemonSet '$name' to be ready..."
    
    local count=0
    while [ $count -lt $timeout ]; do
        local status=$(kubectl get daemonset "$name" -n "$namespace" -o jsonpath='{.status}' 2>/dev/null)
        local desired=$(echo "$status" | jq -r '.desiredNumberScheduled' 2>/dev/null || echo "0")
        local ready=$(echo "$status" | jq -r '.numberReady' 2>/dev/null || echo "0")
        
        if [ "$desired" -eq "$ready" ] && [ "$desired" -gt 0 ]; then
            log_info "DaemonSet '$name' is fully rolled out ($ready/$ready)"
            return 0
        fi
        
        sleep 5
        count=$((count + 5))
    done
    
    log_error "DaemonSet '$name' did not become ready within ${timeout}s"
    return 1
}

# Execute command in a specific node
exec_in_node() {
    local node_name=$1
    local command=$2
    local namespace=${3:-default}
    
    kubectl exec -n "$namespace" "cilium-ipip-router-$(echo "$node_name" | md5sum | cut -c1-11)" -- sh -c "$command"
}

# Get all worker nodes
get_worker_nodes() {
    local workers=$(kubectl get nodes -l node-role.kubernetes.io/worker="" -o jsonpath='{.items[*].metadata.name}')
    if [ -z "$workers" ]; then
        kubectl get nodes -o jsonpath='{.items[*].metadata.name}'
    else
        echo "$workers"
    fi
}

# Get all nodes (control-plane + workers)
get_all_nodes() {
    kubectl get nodes -o jsonpath='{.items[*].metadata.name}'
}

# Collect logs from all router pods
collect_logs() {
    local output_dir=${1:-./e2e-logs}
    
    log_info "Collecting logs to $output_dir..."
    mkdir -p "$output_dir"
    
    local pods=$(kubectl get pods -n "$NAMESPACE" -l app=cilium-ipip-router -o jsonpath='{.items[*].metadata.name}')
    
    for pod in $pods; do
        log_info "Collecting logs from $pod..."
        kubectl logs -n "$NAMESPACE" "$pod" > "$output_dir/${pod}.log" 2>&1 || true
        kubectl logs -n "$NAMESPACE" "$pod" -p >> "$output_dir/${pod}.log" 2>&1 || true
    done
}

# Cleanup function
cleanup() {
    local exit_code=$?
    
    if [ $exit_code -ne 0 ]; then
        log_error "E2E tests failed with exit code $exit_code"
        collect_logs
    fi
    
    exit $exit_code
}

trap cleanup EXIT

# Verify Kind cluster is available
verify_kind_cluster() {
    local cluster_name=${1:-cilium-router-test}
    
    log_info "Verifying Kind cluster '$cluster_name'..."
    
    if ! kind get clusters 2>/dev/null | grep -q "$cluster_name"; then
        log_error "Kind cluster '$cluster_name' not found"
        return 1
    fi
    
    if ! kubectl cluster-info --context "kind-$cluster_name" >/dev/null 2>&1; then
        log_error "Cannot connect to Kind cluster '$cluster_name'"
        return 1
    fi
    
    log_info "Kind cluster '$cluster_name' is available"
    return 0
}

# Set kubeconfig for Kind cluster
setup_kind_kubeconfig() {
    local cluster_name=${1:-cilium-router-test}
    kind get kubeconfig --name "$cluster_name" > /tmp/kubeconfig
    export KUBECONFIG=/tmp/kubeconfig
    log_info "Kubeconfig set for cluster '$cluster_name'"
}
