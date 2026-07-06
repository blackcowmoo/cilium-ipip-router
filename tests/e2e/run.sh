#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

total_tests=0
passed_tests=0
failed_tests=0

run_test() {
    local test_file=$1
    local test_name=$(basename "$test_file" .sh)
    
    echo ""
    echo "========================================"
    echo "Running: $test_name"
    echo "========================================"
    
    ((total_tests++))
    
    if bash "$test_file"; then
        echo -e "${GREEN}✓ PASSED: $test_name${NC}"
        ((passed_tests++))
        return 0
    else
        echo -e "${RED}✗ FAILED: $test_name${NC}"
        ((failed_tests++))
        return 1
    fi
}

main() {
    local cluster_name=${1:-cilium-router-test}
    
    echo "========================================"
    echo "E2E Test Suite"
    echo "========================================"
    echo "Cluster: $cluster_name"
    echo ""
    
    # Check if Kind cluster exists
    if ! kind get clusters 2>/dev/null | grep -q "$cluster_name"; then
        echo -e "${RED}ERROR: Kind cluster '$cluster_name' not found${NC}"
        echo "Create it with: kind create cluster --name $cluster_name"
        exit 1
    fi
    
    # Set kubeconfig
    export KUBECONFIG=$(kind get kubeconfig --name "$cluster_name")
    
    # Verify kubectl connectivity
    if ! kubectl cluster-info >/dev/null 2>&1; then
        echo -e "${RED}ERROR: Cannot connect to Kind cluster${NC}"
        exit 1
    fi
    
    echo "✓ Connected to Kind cluster '$cluster_name'"
    echo ""
    
    # Run individual test scripts
    local failed=0
    
    run_test "$SCRIPT_DIR/test_health.sh" || failed=1
    run_test "$SCRIPT_DIR/test_daemonset.sh" || failed=1
    run_test "$SCRIPT_DIR/test_tunnels.sh" || failed=1
    run_test "$SCRIPT_DIR/test_routes.sh" || failed=1
    run_test "$SCRIPT_DIR/test_node_pods.sh" || failed=1
    run_test "$SCRIPT_DIR/test_lifecycle.sh" || failed=1
    
    # Summary
    echo ""
    echo "========================================"
    echo "Test Summary"
    echo "========================================"
    echo "Total:  $total_tests"
    echo -e "Passed: ${GREEN}$passed_tests${NC}"
    echo -e "Failed: ${RED}$failed_tests${NC}"
    echo ""
    
    if [ $failed -eq 1 ] || [ $failed_tests -gt 0 ]; then
        echo -e "${RED}E2E tests FAILED${NC}"
        exit 1
    fi
    
    echo -e "${GREEN}All E2E tests PASSED${NC}"
    exit 0
}

main "$@"
