#!/bin/bash

echo "=== Testing Email Search Performance ==="
echo ""

# Test 1: Email search
echo "Test: Email Search"
START=$(date +%s%N)
RESPONSE=$(curl -s -w "\n%{http_code}\n%{time_total}" "http://localhost:8080/api/v1/contributor/customer?email=test@example.com")
END=$(date +%s%N)

HTTP_CODE=$(echo "$RESPONSE" | tail -n 2 | head -n 1)
TIME_TOTAL=$(echo "$RESPONSE" | tail -n 1)
BODY=$(echo "$RESPONSE" | head -n -2)

ELAPSED_MS=$(echo "scale=2; $TIME_TOTAL * 1000" | bc)

echo "HTTP Status: $HTTP_CODE"
echo "Response Time: ${ELAPSED_MS}ms"
echo "Response Body:"
echo "$BODY" | jq '.' 2>/dev/null || echo "$BODY"
echo ""

# Check server logs for database error
echo "=== Server Logs (last 20 lines) ==="
tail -20 server.log | grep -A 5 -B 5 "Database error" || echo "No database errors found in logs"
