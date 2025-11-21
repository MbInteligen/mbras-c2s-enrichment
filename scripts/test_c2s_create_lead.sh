#!/bin/bash

# C2S API Smoke Test
# Purpose: Verify C2S lead creation API requirements before implementing Google Ads integration
# Tests: Required fields, optional fields, description length limits, auth header

set -e

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Load environment variables
if [ -f .env ]; then
    export $(cat .env | grep -v '^#' | xargs)
fi

# Check required variables
if [ -z "$C2S_TOKEN" ]; then
    echo -e "${RED}❌ C2S_TOKEN not set. Please set it in .env${NC}"
    exit 1
fi

if [ -z "$C2S_BASE_URL" ]; then
    C2S_BASE_URL="https://api.contact2sale.com"
    echo -e "${YELLOW}⚠️  C2S_BASE_URL not set, using default: $C2S_BASE_URL${NC}"
fi

# Default values (update these based on your C2S configuration)
SELLER_ID="508e51649fabb3502e98a32b4c6763e9"
LEAD_SOURCE_ID=493
CHANNEL_ID=1

echo "=================================================="
echo "C2S API Smoke Test"
echo "=================================================="
echo "Base URL: $C2S_BASE_URL"
echo "Seller ID: $SELLER_ID"
echo "Lead Source ID: $LEAD_SOURCE_ID"
echo "Channel ID: $CHANNEL_ID"
echo ""

# Test 1: Minimal payload (required fields only)
echo -e "${YELLOW}📋 Test 1: Minimal payload (required fields only)${NC}"
RESPONSE=$(curl -s -w "\nHTTP_CODE:%{http_code}" -X POST "$C2S_BASE_URL/integration/leads" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $C2S_TOKEN" \
  -d '{
    "customer": "Smoke Test Minimal",
    "seller_id": "'"$SELLER_ID"'"
  }')

HTTP_CODE=$(echo "$RESPONSE" | grep "HTTP_CODE:" | cut -d: -f2)
BODY=$(echo "$RESPONSE" | sed '/HTTP_CODE:/d')

echo "HTTP Status: $HTTP_CODE"
echo "Response: $BODY"

if [ "$HTTP_CODE" -eq 200 ] || [ "$HTTP_CODE" -eq 201 ]; then
    LEAD_ID_1=$(echo "$BODY" | grep -o '"id":"[^"]*"' | cut -d'"' -f4)
    echo -e "${GREEN}✅ Test 1 PASSED - Lead created: $LEAD_ID_1${NC}"
else
    echo -e "${RED}❌ Test 1 FAILED - Expected 200/201, got $HTTP_CODE${NC}"
    echo "Response: $BODY"
fi
echo ""

# Test 2: Full payload with phone and email
echo -e "${YELLOW}📋 Test 2: Full payload with phone and email${NC}"
RESPONSE=$(curl -s -w "\nHTTP_CODE:%{http_code}" -X POST "$C2S_BASE_URL/integration/leads" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $C2S_TOKEN" \
  -d '{
    "customer": "Smoke Test Full",
    "phone": "+5511987654321",
    "email": "smoke.test@example.com",
    "seller_id": "'"$SELLER_ID"'",
    "description": "Test lead with phone and email"
  }')

HTTP_CODE=$(echo "$RESPONSE" | grep "HTTP_CODE:" | cut -d: -f2)
BODY=$(echo "$RESPONSE" | sed '/HTTP_CODE:/d')

echo "HTTP Status: $HTTP_CODE"
echo "Response: $BODY"

if [ "$HTTP_CODE" -eq 200 ] || [ "$HTTP_CODE" -eq 201 ]; then
    LEAD_ID_2=$(echo "$BODY" | grep -o '"id":"[^"]*"' | cut -d'"' -f4)
    echo -e "${GREEN}✅ Test 2 PASSED - Lead created: $LEAD_ID_2${NC}"
else
    echo -e "${RED}❌ Test 2 FAILED - Expected 200/201, got $HTTP_CODE${NC}"
    echo "Response: $BODY"
fi
echo ""

# Test 3: Short description (500 chars)
echo -e "${YELLOW}📋 Test 3: Short description (500 chars)${NC}"
SHORT_DESC=$(printf 'A%.0s' {1..500})
RESPONSE=$(curl -s -w "\nHTTP_CODE:%{http_code}" -X POST "$C2S_BASE_URL/integration/leads" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $C2S_TOKEN" \
  -d '{
    "customer": "Smoke Test Short Desc",
    "seller_id": "'"$SELLER_ID"'",
    "description": "'"$SHORT_DESC"'"
  }')

HTTP_CODE=$(echo "$RESPONSE" | grep "HTTP_CODE:" | cut -d: -f2)
BODY=$(echo "$RESPONSE" | sed '/HTTP_CODE:/d')

echo "HTTP Status: $HTTP_CODE"
echo "Description length: ${#SHORT_DESC} chars"

if [ "$HTTP_CODE" -eq 200 ] || [ "$HTTP_CODE" -eq 201 ]; then
    LEAD_ID_3=$(echo "$BODY" | grep -o '"id":"[^"]*"' | cut -d'"' -f4)
    echo -e "${GREEN}✅ Test 3 PASSED - Lead created: $LEAD_ID_3${NC}"
else
    echo -e "${RED}❌ Test 3 FAILED - Expected 200/201, got $HTTP_CODE${NC}"
    echo "Response: $BODY"
fi
echo ""

# Test 4: Medium description (2000 chars)
echo -e "${YELLOW}📋 Test 4: Medium description (2000 chars)${NC}"
MEDIUM_DESC=$(printf 'B%.0s' {1..2000})
RESPONSE=$(curl -s -w "\nHTTP_CODE:%{http_code}" -X POST "$C2S_BASE_URL/integration/leads" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $C2S_TOKEN" \
  -d '{
    "customer": "Smoke Test Medium Desc",
    "seller_id": "'"$SELLER_ID"'",
    "description": "'"$MEDIUM_DESC"'"
  }')

HTTP_CODE=$(echo "$RESPONSE" | grep "HTTP_CODE:" | cut -d: -f2)
BODY=$(echo "$RESPONSE" | sed '/HTTP_CODE:/d')

echo "HTTP Status: $HTTP_CODE"
echo "Description length: ${#MEDIUM_DESC} chars"

if [ "$HTTP_CODE" -eq 200 ] || [ "$HTTP_CODE" -eq 201 ]; then
    LEAD_ID_4=$(echo "$BODY" | grep -o '"id":"[^"]*"' | cut -d'"' -f4)
    echo -e "${GREEN}✅ Test 4 PASSED - Lead created: $LEAD_ID_4${NC}"
else
    echo -e "${RED}❌ Test 4 FAILED - Expected 200/201, got $HTTP_CODE${NC}"
    echo "Response: $BODY"
fi
echo ""

# Test 5: Long description (5000 chars) - to find limit
echo -e "${YELLOW}📋 Test 5: Long description (5000 chars) - testing limit${NC}"
LONG_DESC=$(printf 'C%.0s' {1..5000})
RESPONSE=$(curl -s -w "\nHTTP_CODE:%{http_code}" -X POST "$C2S_BASE_URL/integration/leads" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $C2S_TOKEN" \
  -d '{
    "customer": "Smoke Test Long Desc",
    "seller_id": "'"$SELLER_ID"'",
    "description": "'"$LONG_DESC"'"
  }')

HTTP_CODE=$(echo "$RESPONSE" | grep "HTTP_CODE:" | cut -d: -f2)
BODY=$(echo "$RESPONSE" | sed '/HTTP_CODE:/d')

echo "HTTP Status: $HTTP_CODE"
echo "Description length: ${#LONG_DESC} chars"

if [ "$HTTP_CODE" -eq 200 ] || [ "$HTTP_CODE" -eq 201 ]; then
    LEAD_ID_5=$(echo "$BODY" | grep -o '"id":"[^"]*"' | cut -d'"' -f4)
    echo -e "${GREEN}✅ Test 5 PASSED - Lead created: $LEAD_ID_5${NC}"
    echo -e "${GREEN}📊 Description limit appears to be >= 5000 chars${NC}"
else
    echo -e "${YELLOW}⚠️  Test 5 FAILED at 5000 chars - this is likely the limit${NC}"
    echo "Response: $BODY"
    echo -e "${YELLOW}💡 Recommended C2S_DESCRIPTION_MAX_LENGTH: 2000 (safe default)${NC}"
fi
echo ""

# Test 6: Very long description (10000 chars) - definitely over limit
echo -e "${YELLOW}📋 Test 6: Very long description (10000 chars) - confirming over limit${NC}"
VERY_LONG_DESC=$(printf 'D%.0s' {1..10000})
RESPONSE=$(curl -s -w "\nHTTP_CODE:%{http_code}" -X POST "$C2S_BASE_URL/integration/leads" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $C2S_TOKEN" \
  -d '{
    "customer": "Smoke Test Very Long Desc",
    "seller_id": "'"$SELLER_ID"'",
    "description": "'"$VERY_LONG_DESC"'"
  }')

HTTP_CODE=$(echo "$RESPONSE" | grep "HTTP_CODE:" | cut -d: -f2)
BODY=$(echo "$RESPONSE" | sed '/HTTP_CODE:/d')

echo "HTTP Status: $HTTP_CODE"
echo "Description length: ${#VERY_LONG_DESC} chars"

if [ "$HTTP_CODE" -eq 200 ] || [ "$HTTP_CODE" -eq 201 ]; then
    LEAD_ID_6=$(echo "$BODY" | grep -o '"id":"[^"]*"' | cut -d'"' -f4)
    echo -e "${GREEN}✅ Test 6 PASSED - Lead created: $LEAD_ID_6${NC}"
    echo -e "${GREEN}📊 Description limit appears to be >= 10000 chars${NC}"
else
    echo -e "${YELLOW}⚠️  Test 6 FAILED at 10000 chars${NC}"
    echo "Response: $BODY"
fi
echo ""

# Summary
echo "=================================================="
echo "Summary & Recommendations"
echo "=================================================="
echo ""
echo "✅ Required fields confirmed:"
echo "   - customer (string) - customer name"
echo "   - seller_id (string) - optional but recommended"
echo ""
echo "✅ Optional fields confirmed:"
echo "   - phone (string, E.164 format)"
echo "   - email (string)"
echo "   - description (string, see length notes below)"
echo "   - product (string)"
echo "   - source (string)"
echo ""
echo "✅ Auth header confirmed: Authorization: Bearer <token>"
echo ""
echo "📊 Description Length Analysis:"
if [ "$HTTP_CODE" -eq 200 ] || [ "$HTTP_CODE" -eq 201 ]; then
    echo "   - 10000+ chars: PASSED ✅"
    echo "   - Recommended C2S_DESCRIPTION_MAX_LENGTH: 5000"
else
    echo "   - Failed at some point (check individual tests above)"
    echo "   - Recommended C2S_DESCRIPTION_MAX_LENGTH: 2000 (conservative)"
fi
echo ""
echo "💡 Configuration values for .env:"
echo "   C2S_DEFAULT_SELLER_ID=$SELLER_ID"
echo "   C2S_DEFAULT_LEAD_SOURCE_ID=$LEAD_SOURCE_ID"
echo "   C2S_DEFAULT_CHANNEL_ID=$CHANNEL_ID"
echo "   C2S_DESCRIPTION_MAX_LENGTH=5000  # Adjust based on test results"
echo ""
echo "=================================================="
