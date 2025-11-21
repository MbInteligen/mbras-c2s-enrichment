#!/bin/bash

# C2S Gateway Integration Test Script
# Tests the integration between Rust API and Python C2S Gateway

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
GATEWAY_URL="${C2S_GATEWAY_URL:-https://mbras-c2s-gateway.fly.dev}"
RUST_API_URL="${RUST_API_URL:-http://localhost:8080}"
TEST_LEAD_ID="bf1a88eaa4ab34b01a257536563fb42b"  # Guilherme Cappi

echo "=========================================="
echo "C2S Gateway Integration Test Suite"
echo "=========================================="
echo ""
echo "Gateway URL: $GATEWAY_URL"
echo "Rust API URL: $RUST_API_URL"
echo ""

# Track test results
TESTS_PASSED=0
TESTS_FAILED=0

# Test function
run_test() {
    local test_name=$1
    local command=$2
    local expected_field=$3

    echo -n "Testing: $test_name ... "

    if output=$(eval "$command" 2>&1); then
        if [ -n "$expected_field" ]; then
            if echo "$output" | grep -q "$expected_field"; then
                echo -e "${GREEN}PASS${NC}"
                TESTS_PASSED=$((TESTS_PASSED + 1))
                return 0
            else
                echo -e "${RED}FAIL${NC} (field '$expected_field' not found)"
                echo "Output: $output" | head -3
                TESTS_FAILED=$((TESTS_FAILED + 1))
                return 1
            fi
        else
            echo -e "${GREEN}PASS${NC}"
            TESTS_PASSED=$((TESTS_PASSED + 1))
            return 0
        fi
    else
        echo -e "${RED}FAIL${NC}"
        echo "Error: $output" | head -3
        TESTS_FAILED=$((TESTS_FAILED + 1))
        return 1
    fi
}

echo "=== Phase 1: Gateway Direct Tests ==="
echo ""

# Test 1: Gateway Health Check
run_test "Gateway Health Check" \
    "curl -s $GATEWAY_URL" \
    "C2S Gateway"

# Test 2: Gateway Can Fetch Lead
run_test "Gateway Fetch Lead" \
    "curl -s $GATEWAY_URL/leads/$TEST_LEAD_ID" \
    "Guilherme"

# Test 3: Gateway API Docs Available
run_test "Gateway API Docs" \
    "curl -s -o /dev/null -w '%{http_code}' $GATEWAY_URL/docs" \
    "200"

echo ""
echo "=== Phase 2: Rust API Integration Tests ==="
echo ""

# Test 4: Rust API Health Check
run_test "Rust API Health" \
    "curl -s $RUST_API_URL/health" \
    "healthy"

# Test 5: Rust API Smoke Test Endpoint
run_test "Rust API Gateway Smoke Test" \
    "curl -s $RUST_API_URL/test-gateway" \
    "connectivity.*success"

# Test 6: Verify Gateway URL in Response
run_test "Gateway URL Configured" \
    "curl -s $RUST_API_URL/test-gateway | grep -o 'mbras-c2s-gateway.fly.dev'" \
    "mbras-c2s-gateway.fly.dev"

echo ""
echo "=== Phase 3: End-to-End Tests ==="
echo ""

# Test 7: Rust API Can Call Gateway
if command -v jq >/dev/null 2>&1; then
    response=$(curl -s $RUST_API_URL/test-gateway)
    status=$(echo "$response" | jq -r '.status' 2>/dev/null)

    if [ "$status" = "200" ]; then
        echo -e "Rust → Gateway Communication: ${GREEN}PASS${NC}"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "Rust → Gateway Communication: ${RED}FAIL${NC} (status: $status)"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
else
    echo -e "${YELLOW}SKIP${NC} End-to-end test (jq not installed)"
fi

echo ""
echo "=== Phase 4: Performance Test ==="
echo ""

# Test 8: Response Time Check
start_time=$(date +%s%N)
curl -s $GATEWAY_URL > /dev/null 2>&1
end_time=$(date +%s%N)
elapsed=$(( (end_time - start_time) / 1000000 ))

if [ $elapsed -lt 1000 ]; then
    echo -e "Gateway Response Time (${elapsed}ms): ${GREEN}PASS${NC}"
    TESTS_PASSED=$((TESTS_PASSED + 1))
else
    echo -e "Gateway Response Time (${elapsed}ms): ${YELLOW}SLOW${NC}"
fi

echo ""
echo "=========================================="
echo "Test Results"
echo "=========================================="
echo -e "Passed: ${GREEN}$TESTS_PASSED${NC}"
echo -e "Failed: ${RED}$TESTS_FAILED${NC}"
echo ""

if [ $TESTS_FAILED -eq 0 ]; then
    echo -e "${GREEN}✓ All tests passed!${NC}"
    echo ""
    echo "Integration Status: READY"
    echo "You can now migrate endpoints to use the gateway."
    exit 0
else
    echo -e "${RED}✗ Some tests failed${NC}"
    echo ""
    echo "Please check the failures above before proceeding."
    exit 1
fi
