#!/bin/bash
# Local testing script for rust-c2s-api
# Usage: ./test-local.sh [base_url]
# Example: ./test-local.sh http://localhost:8081

set -e

BASE_URL="${1:-http://localhost:8081}"
LEAD_ID="${2:-358f62821dc6cfa7cfbda19e670d6392}"

echo "🧪 Testing rust-c2s-api at $BASE_URL"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

test_endpoint() {
    local name="$1"
    local method="$2"
    local endpoint="$3"
    local data="$4"

    echo ""
    echo -e "${YELLOW}Testing: $name${NC}"
    echo "  → $method $endpoint"

    if [ "$method" = "GET" ]; then
        response=$(curl -s -w "\n%{http_code}" "$BASE_URL$endpoint")
    else
        response=$(curl -s -w "\n%{http_code}" -X "$method" \
            -H "Content-Type: application/json" \
            -d "$data" \
            "$BASE_URL$endpoint")
    fi

    http_code=$(echo "$response" | tail -n1)
    body=$(echo "$response" | sed '$d')

    if [ "$http_code" -ge 200 ] && [ "$http_code" -lt 300 ]; then
        echo -e "  ${GREEN}✓ HTTP $http_code${NC}"
        echo "$body" | jq '.' 2>/dev/null || echo "$body"
    else
        echo -e "  ${RED}✗ HTTP $http_code${NC}"
        echo "$body" | jq '.' 2>/dev/null || echo "$body"
    fi
}

# 1. Health Check
test_endpoint "Health Check" "GET" "/health" ""

# 2. Trigger Lead Processing (Main Make.com endpoint)
test_endpoint "Trigger Lead Processing" "GET" "/api/v1/leads/process?id=$LEAD_ID" ""

# 3. Get Customer by CPF
test_endpoint "Get Customer by CPF" "GET" "/api/v1/contributor/customer?cpf=12345678900" ""

# 4. Get Customer by Email
test_endpoint "Get Customer by Email" "GET" "/api/v1/contributor/customer?email=test@example.com" ""

# 5. Get Customer by Phone
test_endpoint "Get Customer by Phone" "GET" "/api/v1/contributor/customer?phone=5511999998888" ""

# 6. Get Customer by Name
test_endpoint "Get Customer by Name" "GET" "/api/v1/contributor/customer?name=João Silva" ""

# 7. Enrich Customer (POST with JSON)
ENRICH_PAYLOAD='{
  "cpf": "12345678900",
  "email": "test@example.com",
  "phone": "5511999998888",
  "name": "João Silva"
}'
test_endpoint "Enrich Customer" "POST" "/api/v1/enrich" "$ENRICH_PAYLOAD"

# 8. Work API - Fetch All Modules
test_endpoint "Work API All Modules" "GET" "/api/v1/work/modules/all?documento=12345678900" ""

# 9. Work API - Fetch Single Module
test_endpoint "Work API CPF Module" "GET" "/api/v1/work/modules/cpf?documento=12345678900" ""

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo -e "${GREEN}✓ Testing complete!${NC}"
echo ""
echo "💡 Tips:"
echo "  - Check logs with: fly logs (if testing Fly.io deployment)"
echo "  - Monitor resources: fly status --app rust-c2s-api"
echo "  - Run load tests: k6 run tests/load-test.js"
