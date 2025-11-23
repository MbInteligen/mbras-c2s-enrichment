#!/bin/bash

echo "═══════════════════════════════════════════════════════════════"
echo "  Comprehensive API Endpoint Test - All Results"
echo "═══════════════════════════════════════════════════════════════"
echo ""

BASE_URL="http://localhost:8080"
TOTAL=0
PASSED=0
FAILED=0

test_endpoint() {
    local name="$1"
    local method="$2"
    local endpoint="$3"
    local expected_status="$4"
    local data="$5"
    
    TOTAL=$((TOTAL + 1))
    
    echo "────────────────────────────────────────────────────────────────"
    printf "Test %2d: %-50s\n" $TOTAL "$name"
    echo "────────────────────────────────────────────────────────────────"
    
    # Run request and capture timing
    START=$(date +%s%N)
    
    if [ "$method" = "POST" ]; then
        RESULT=$(curl -s -w "\nHTTP_STATUS:%{http_code}" -X POST \
            -H "Content-Type: application/json" \
            -d "$data" \
            "${BASE_URL}${endpoint}" 2>&1)
    else
        RESULT=$(curl -s -w "\nHTTP_STATUS:%{http_code}" "${BASE_URL}${endpoint}" 2>&1)
    fi
    
    END=$(date +%s%N)
    TIME_MS=$(( (END - START) / 1000000 ))
    
    # Extract HTTP status
    HTTP_CODE=$(echo "$RESULT" | grep "HTTP_STATUS:" | cut -d: -f2)
    BODY=$(echo "$RESULT" | grep -v "HTTP_STATUS:")
    
    echo "Method:   $method"
    echo "Endpoint: $endpoint"
    echo "Expected: HTTP $expected_status"
    echo "Actual:   HTTP $HTTP_CODE"
    echo "Time:     ${TIME_MS}ms"
    
    # Determine pass/fail
    if [ "$HTTP_CODE" = "$expected_status" ]; then
        echo "Result:   ✓ PASS"
        PASSED=$((PASSED + 1))
    else
        echo "Result:   ✗ FAIL"
        FAILED=$((FAILED + 1))
    fi
    
    echo ""
    echo "Response:"
    echo "$BODY" | jq '.' 2>/dev/null || echo "$BODY" | head -20
    echo ""
}

# 1. Health Check
test_endpoint "Health Check" "GET" "/health" "200"

# 2. CPF Search - Valid  
test_endpoint "CPF Search (Valid CPF)" "GET" "/api/v1/contributor/customer?cpf=12345678901" "200"

# 3. Email Search - Valid
test_endpoint "Email Search (Valid)" "GET" "/api/v1/contributor/customer?email=test@example.com" "200"

# 4. Email Search - Not Found
test_endpoint "Email Search (Not Found)" "GET" "/api/v1/contributor/customer?email=notfound@example.com" "404"

# 5. Phone Search
test_endpoint "Phone Search" "GET" "/api/v1/contributor/customer?phone=11999999999" "200"

# 6. Work API - All Modules
test_endpoint "Work API - All Modules" "GET" "/api/v1/work/modules/all?documento=12345678901" "200"

# 7. Work API - DadosBasicos
test_endpoint "Work API - DadosBasicos Module" "GET" "/api/v1/work/modules/DadosBasicos?documento=12345678901" "200"

# 8. Work API - CEP
test_endpoint "Work API - CEP Lookup" "GET" "/api/v1/work/modules/cep?documento=05676120" "200"

# 9. Lead Processing - No ID
test_endpoint "Lead Processing (Missing ID)" "GET" "/api/v1/leads/process" "400"

# 10. Lead Processing - With ID
test_endpoint "Lead Processing (With ID)" "GET" "/api/v1/leads/process?id=test123" "200"

# 11. Google Ads Webhook - No Key
test_endpoint "Google Ads Webhook (No Key)" "POST" "/api/v1/webhooks/google-ads" "401" '{"gclid":"test","name":"Test","email":"test@example.com","phone":"11999999999"}'

# 12. Google Ads Webhook - Wrong Key
test_endpoint "Google Ads Webhook (Wrong Key)" "POST" "/api/v1/webhooks/google-ads?google_key=wrong" "401" '{"gclid":"test","name":"Test","email":"test@example.com","phone":"11999999999"}'

echo "═══════════════════════════════════════════════════════════════"
echo "  SUMMARY"
echo "═══════════════════════════════════════════════════════════════"
echo ""
printf "%-20s %3d\n" "Total Tests:" $TOTAL
printf "%-20s %3d (%.1f%%)\n" "Passed:" $PASSED $(echo "scale=1; $PASSED * 100 / $TOTAL" | bc)
printf "%-20s %3d (%.1f%%)\n" "Failed:" $FAILED $(echo "scale=1; $FAILED * 100 / $TOTAL" | bc)
echo ""

if [ $FAILED -eq 0 ]; then
    echo "🎉 ALL TESTS PASSED!"
else
    echo "⚠️  $FAILED test(s) failed"
fi
echo ""
echo "═══════════════════════════════════════════════════════════════"
