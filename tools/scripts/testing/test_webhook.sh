#!/bin/bash
# Test script for C2S webhook endpoint
# Tests single event, batch events, authentication, and idempotency

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
BASE_URL="${1:-http://localhost:8080}"
WEBHOOK_SECRET="${WEBHOOK_SECRET:-test-secret-12345}"

echo "=================================================="
echo "C2S Webhook Endpoint Tests"
echo "=================================================="
echo "Base URL: $BASE_URL"
echo "Webhook Secret: ${WEBHOOK_SECRET:0:10}..."
echo ""

# Test counter
TESTS_PASSED=0
TESTS_FAILED=0

# Helper function to test endpoint
test_webhook() {
    local test_name="$1"
    local payload="$2"
    local expected_status="$3"
    local use_auth="${4:-yes}"

    echo -n "Testing: $test_name... "

    # Build curl command
    local curl_cmd="curl -s -w '\n%{http_code}' -X POST"
    curl_cmd="$curl_cmd -H 'Content-Type: application/json'"

    if [ "$use_auth" = "yes" ]; then
        curl_cmd="$curl_cmd -H 'X-Webhook-Token: $WEBHOOK_SECRET'"
    fi

    curl_cmd="$curl_cmd -d '$payload' $BASE_URL/api/v1/webhooks/c2s"

    # Execute request
    response=$(eval $curl_cmd)

    # Extract status code (last line)
    status_code=$(echo "$response" | tail -n1)
    body=$(echo "$response" | head -n-1)

    # Check status code
    if [ "$status_code" = "$expected_status" ]; then
        echo -e "${GREEN}✓ PASS${NC} (HTTP $status_code)"
        TESTS_PASSED=$((TESTS_PASSED + 1))

        # Show response body if successful
        if [ "$status_code" = "200" ]; then
            echo "  Response: $body"
        fi
        return 0
    else
        echo -e "${RED}✗ FAIL${NC} (Expected $expected_status, got $status_code)"
        echo "  Response: $body"
        TESTS_FAILED=$((TESTS_FAILED + 1))
        return 1
    fi
}

echo "Phase 1: Authentication Tests"
echo "----------------------------------------------"

# Test 1: Missing auth header (should fail if WEBHOOK_SECRET is configured)
test_webhook \
    "Missing authentication header" \
    '{"id":"test-001","attributes":{"updated_at":"2025-01-20T10:00:00Z"}}' \
    "401" \
    "no"

# Test 2: Invalid auth token
curl -s -w '\n%{http_code}' -X POST \
    -H 'Content-Type: application/json' \
    -H 'X-Webhook-Token: wrong-secret' \
    -d '{"id":"test-002","attributes":{"updated_at":"2025-01-20T10:00:00Z"}}' \
    "$BASE_URL/api/v1/webhooks/c2s" > /tmp/webhook_test_invalid_auth.txt 2>&1

status_code=$(tail -n1 /tmp/webhook_test_invalid_auth.txt)
if [ "$status_code" = "401" ]; then
    echo -e "Testing: Invalid authentication token... ${GREEN}✓ PASS${NC} (HTTP 401)"
    TESTS_PASSED=$((TESTS_PASSED + 1))
else
    echo -e "Testing: Invalid authentication token... ${RED}✗ FAIL${NC} (Expected 401, got $status_code)"
    TESTS_FAILED=$((TESTS_FAILED + 1))
fi

echo ""
echo "Phase 2: Single Event Tests"
echo "----------------------------------------------"

# Test 3: Valid single event
TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
test_webhook \
    "Valid single event" \
    "{
        \"id\": \"lead-12345\",
        \"hook_action\": \"lead.created\",
        \"attributes\": {
            \"updated_at\": \"$TIMESTAMP\",
            \"customer\": {
                \"name\": \"João Silva\",
                \"email\": \"joao@example.com\",
                \"phone\": \"+5511987654321\"
            },
            \"product\": {
                \"description\": \"Apartamento 3 quartos\",
                \"prop_ref\": \"APT-001\",
                \"price\": \"R$ 500.000\"
            },
            \"lead_status\": {
                \"alias\": \"new\",
                \"name\": \"Novo Lead\"
            }
        }
    }" \
    "200" \
    "yes"

# Test 4: Duplicate event (same lead_id + updated_at)
sleep 1
test_webhook \
    "Duplicate event (idempotency)" \
    "{
        \"id\": \"lead-12345\",
        \"hook_action\": \"lead.created\",
        \"attributes\": {
            \"updated_at\": \"$TIMESTAMP\",
            \"customer\": {
                \"name\": \"João Silva\",
                \"email\": \"joao@example.com\"
            }
        }
    }" \
    "200" \
    "yes"

# Test 5: Same lead_id but different updated_at (should be new event)
TIMESTAMP2=$(date -u -v+1M +"%Y-%m-%dT%H:%M:%SZ" 2>/dev/null || date -u -d "+1 minute" +"%Y-%m-%dT%H:%M:%SZ")
test_webhook \
    "Same lead, different timestamp" \
    "{
        \"id\": \"lead-12345\",
        \"hook_action\": \"lead.updated\",
        \"attributes\": {
            \"updated_at\": \"$TIMESTAMP2\",
            \"customer\": {
                \"name\": \"João Silva Updated\"
            }
        }
    }" \
    "200" \
    "yes"

echo ""
echo "Phase 3: Batch Event Tests"
echo "----------------------------------------------"

# Test 6: Batch of events
TIMESTAMP3=$(date -u -v+2M +"%Y-%m-%dT%H:%M:%SZ" 2>/dev/null || date -u -d "+2 minutes" +"%Y-%m-%dT%H:%M:%SZ")
TIMESTAMP4=$(date -u -v+3M +"%Y-%m-%dT%H:%M:%SZ" 2>/dev/null || date -u -d "+3 minutes" +"%Y-%m-%dT%H:%M:%SZ")

test_webhook \
    "Batch of 3 events" \
    "[
        {
            \"id\": \"lead-batch-1\",
            \"attributes\": {
                \"updated_at\": \"$TIMESTAMP3\",
                \"customer\": {\"name\": \"Maria Santos\"}
            }
        },
        {
            \"id\": \"lead-batch-2\",
            \"attributes\": {
                \"updated_at\": \"$TIMESTAMP3\",
                \"customer\": {\"name\": \"Carlos Oliveira\"}
            }
        },
        {
            \"id\": \"lead-batch-3\",
            \"attributes\": {
                \"updated_at\": \"$TIMESTAMP4\",
                \"customer\": {\"name\": \"Ana Costa\"}
            }
        }
    ]" \
    "200" \
    "yes"

echo ""
echo "Phase 4: Error Handling Tests"
echo "----------------------------------------------"

# Test 7: Missing updated_at
test_webhook \
    "Missing updated_at field" \
    "{
        \"id\": \"lead-error-1\",
        \"attributes\": {
            \"customer\": {\"name\": \"Test User\"}
        }
    }" \
    "400" \
    "yes"

# Test 8: Invalid timestamp format
test_webhook \
    "Invalid timestamp format" \
    "{
        \"id\": \"lead-error-2\",
        \"attributes\": {
            \"updated_at\": \"invalid-date\",
            \"customer\": {\"name\": \"Test User\"}
        }
    }" \
    "400" \
    "yes"

echo ""
echo "=================================================="
echo "Test Summary"
echo "=================================================="
echo -e "Tests Passed: ${GREEN}$TESTS_PASSED${NC}"
echo -e "Tests Failed: ${RED}$TESTS_FAILED${NC}"
echo ""

if [ $TESTS_FAILED -eq 0 ]; then
    echo -e "${GREEN}All tests passed!${NC}"
    exit 0
else
    echo -e "${RED}Some tests failed.${NC}"
    exit 1
fi
