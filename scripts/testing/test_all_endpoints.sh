#!/bin/bash

echo "═══════════════════════════════════════════════════════════════"
echo "  Comprehensive API Endpoint Test"
echo "  Testing all 17 endpoints with performance metrics"
echo "═══════════════════════════════════════════════════════════════"
echo ""

BASE_URL="http://localhost:8080"
TOTAL=0
PASSED=0
FAILED=0

# Color codes
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

test_endpoint() {
    local name="$1"
    local method="$2"
    local endpoint="$3"
    local expected_status="$4"
    local data="$5"
    
    TOTAL=$((TOTAL + 1))
    
    echo "────────────────────────────────────────────────────────────────"
    printf "Test %2d: %s\n" $TOTAL "$name"
    echo "────────────────────────────────────────────────────────────────"
    
    if [ "$method" = "POST" ]; then
        RESPONSE=$(curl -s -w "\n%{http_code}\n%{time_total}" -X POST \
            -H "Content-Type: application/json" \
            -d "$data" \
            "${BASE_URL}${endpoint}")
    else
        RESPONSE=$(curl -s -w "\n%{http_code}\n%{time_total}" "${BASE_URL}${endpoint}")
    fi
    
    HTTP_CODE=$(echo "$RESPONSE" | tail -n 2 | head -n 1)
    TIME_TOTAL=$(echo "$RESPONSE" | tail -n 1)
    BODY=$(echo "$RESPONSE" | head -n -2)
    
    TIME_MS=$(echo "$TIME_TOTAL * 1000" | bc | cut -d. -f1)
    
    echo "Endpoint: $method $endpoint"
    echo "Expected: HTTP $expected_status"
    echo "Actual:   HTTP $HTTP_CODE"
    echo "Time:     ${TIME_MS}ms"
    
    if [ "$HTTP_CODE" = "$expected_status" ]; then
        echo -e "Result:   ${GREEN}✓ PASS${NC}"
        PASSED=$((PASSED + 1))
    else
        echo -e "Result:   ${RED}✗ FAIL${NC}"
        FAILED=$((FAILED + 1))
    fi
    
    echo ""
    echo "Response Body:"
    echo "$BODY" | jq '.' 2>/dev/null || echo "$BODY"
    echo ""
}

# 1. Health Check
test_endpoint "Health Check" "GET" "/health" "200"

# 2. CPF Search - Valid
test_endpoint "CPF Search (Valid)" "GET" "/api/v1/contributor/customer?cpf=12345678901" "200"

# 3. CPF Search - Not Found
test_endpoint "CPF Search (Not Found)" "GET" "/api/v1/contributor/customer?cpf=99999999999" "404"

# 4. Email Search - Valid
test_endpoint "Email Search (Valid)" "GET" "/api/v1/contributor/customer?email=test@example.com" "200"

# 5. Email Search - Not Found
test_endpoint "Email Search (Not Found)" "GET" "/api/v1/contributor/customer?email=notfound@example.com" "404"

# 6. Phone Search - Valid
test_endpoint "Phone Search (Valid)" "GET" "/api/v1/contributor/customer?phone=11999999999" "200"

# 7. Phone Search - Not Found
test_endpoint "Phone Search (Not Found)" "GET" "/api/v1/contributor/customer?phone=11000000000" "404"

# 8. Work API - All Modules (should work even with invalid CPF for testing)
test_endpoint "Work API All Modules" "GET" "/api/v1/work/modules/all?documento=12345678901" "200"

# 9. Work API - DadosBasicos Module
test_endpoint "Work API DadosBasicos" "GET" "/api/v1/work/modules/DadosBasicos?documento=12345678901" "200"

# 10. Work API - Invalid Module
test_endpoint "Work API Invalid Module" "GET" "/api/v1/work/modules/InvalidModule?documento=12345678901" "400"

# 11. Work API - CEP Lookup
test_endpoint "Work API CEP Lookup" "GET" "/api/v1/work/modules/cep?documento=05676120" "200"

# 12. Post Lead (basic validation)
LEAD_DATA='{"lead_id":"test123","personal_info":{"name":"Test User","cpf":"12345678901"},"contact_info":{"email":"test@example.com","phone":"11999999999"}}'
test_endpoint "Post Lead" "POST" "/api/v1/leads" "200" "$LEAD_DATA"

# 13. Lead Processing Endpoint (Make.com integration)
test_endpoint "Lead Processing (no ID)" "GET" "/api/v1/leads/process" "400"

# 14. Lead Processing with ID
test_endpoint "Lead Processing (with ID)" "GET" "/api/v1/leads/process?id=test123" "200"

# 15. C2S Enrich Endpoint (will fail without valid lead_id)
test_endpoint "C2S Enrich (invalid ID)" "POST" "/api/v1/c2s/enrich/invalid_lead_id" "500"

# 16. Google Ads Webhook - Wrong Key
test_endpoint "Google Ads Webhook (Wrong Key)" "POST" "/api/v1/webhooks/google-ads?google_key=wrong" "401" '{"gclid":"test","name":"Test","email":"test@example.com","phone":"11999999999"}'

# 17. Google Ads Webhook - Missing Key
test_endpoint "Google Ads Webhook (No Key)" "POST" "/api/v1/webhooks/google-ads" "401" '{"gclid":"test","name":"Test","email":"test@example.com","phone":"11999999999"}'

echo "═══════════════════════════════════════════════════════════════"
echo "  FINAL RESULTS"
echo "═══════════════════════════════════════════════════════════════"
echo ""
echo "Total Tests:    $TOTAL"
echo -e "Passed:         ${GREEN}$PASSED${NC}"
echo -e "Failed:         ${RED}$FAILED${NC}"

SUCCESS_RATE=$(echo "scale=1; $PASSED * 100 / $TOTAL" | bc)
echo "Success Rate:   ${SUCCESS_RATE}%"
echo ""

if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}🎉 ALL TESTS PASSED!${NC}"
else
    echo -e "${YELLOW}⚠️  Some tests failed - review results above${NC}"
fi

echo ""
echo "═══════════════════════════════════════════════════════════════"
