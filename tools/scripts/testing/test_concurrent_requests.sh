#!/bin/bash

# Test script to simulate concurrent duplicate requests
# This tests the deduplication logic

LEAD_ID="${1:-test-lead-12345}"
API_URL="${2:-https://mbras-c2s.fly.dev}"
CONCURRENT_REQUESTS="${3:-3}"

echo "=================================================="
echo "Testing Concurrent Request Deduplication"
echo "=================================================="
echo "Lead ID: $LEAD_ID"
echo "API URL: $API_URL"
echo "Concurrent Requests: $CONCURRENT_REQUESTS"
echo "=================================================="
echo ""

# Function to make a request
make_request() {
    local request_num=$1
    local start_time=$(date +%s%3N)

    echo "[Request $request_num] Starting at $(date +%H:%M:%S.%3N)"

    response=$(curl -s -w "\n%{http_code}" \
        "${API_URL}/api/v1/leads/process?id=${LEAD_ID}")

    http_code=$(echo "$response" | tail -n1)
    body=$(echo "$response" | head -n-1)

    local end_time=$(date +%s%3N)
    local duration=$((end_time - start_time))

    echo "[Request $request_num] Completed in ${duration}ms - HTTP $http_code"
    echo "[Request $request_num] Response: $body" | jq '.' 2>/dev/null || echo "$body"
    echo ""
}

# Launch concurrent requests
pids=()
for i in $(seq 1 $CONCURRENT_REQUESTS); do
    make_request $i &
    pids+=($!)
    # Small stagger to simulate near-simultaneous requests
    sleep 0.05
done

echo "Waiting for all requests to complete..."
echo ""

# Wait for all background jobs
for pid in "${pids[@]}"; do
    wait $pid
done

echo "=================================================="
echo "All requests completed!"
echo "=================================================="
echo ""
echo "Expected behavior:"
echo "  - Only 1 request should process successfully"
echo "  - Other $(($CONCURRENT_REQUESTS - 1)) should be blocked with 'duplicate_request: true'"
echo ""
