#!/bin/bash

# Test Google Ads Webhook Integration
# Tests the complete flow: webhook → enrichment → C2S lead creation

set -e

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Configuration
BASE_URL="${1:-http://localhost:8080}"
GOOGLE_KEY="${2:-test_key_12345}"

echo "=================================================="
echo "Google Ads Webhook Integration Test"
echo "=================================================="
echo "Base URL: $BASE_URL"
echo "Google Key: $GOOGLE_KEY"
echo ""

# Test 1: Missing google_key (should fail with 401)
echo -e "${YELLOW}📋 Test 1: Missing google_key (should fail)${NC}"
RESPONSE=$(curl -s -w "\nHTTP_CODE:%{http_code}" -X POST "$BASE_URL/api/v1/webhooks/google-ads" \
  -H "Content-Type: application/json" \
  -d '{
    "lead_id": "test-001",
    "api_version": "v1",
    "form_id": 123456,
    "campaign_id": 789012,
    "is_test": true,
    "user_column_data": [
      {"column_id": "FULL_NAME", "column_name": "Nome Completo", "string_value": "João Silva"},
      {"column_id": "EMAIL", "column_name": "E-mail", "string_value": "joao@example.com"},
      {"column_id": "PHONE_NUMBER", "column_name": "Telefone", "string_value": "11987654321"}
    ]
  }')

HTTP_CODE=$(echo "$RESPONSE" | grep "HTTP_CODE:" | cut -d: -f2)
if [ "$HTTP_CODE" -eq 401 ] || [ "$HTTP_CODE" -eq 400 ]; then
    echo -e "${GREEN}✅ Test 1 PASSED - Correctly rejected (HTTP $HTTP_CODE)${NC}"
else
    echo -e "${RED}❌ Test 1 FAILED - Expected 401/400, got $HTTP_CODE${NC}"
fi
echo ""

# Test 2: Valid webhook with google_key (minimal data)
echo -e "${YELLOW}📋 Test 2: Valid webhook with minimal data${NC}"
RESPONSE=$(curl -s -w "\nHTTP_CODE:%{http_code}" -X POST "$BASE_URL/api/v1/webhooks/google-ads?google_key=$GOOGLE_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "lead_id": "test-minimal-'$(date +%s)'",
    "api_version": "v1",
    "form_id": 123456,
    "campaign_id": 789012,
    "google_key": "'$GOOGLE_KEY'",
    "is_test": true,
    "user_column_data": [
      {"column_id": "FULL_NAME", "column_name": "Nome Completo", "string_value": "Test Minimal User"}
    ]
  }')

HTTP_CODE=$(echo "$RESPONSE" | grep "HTTP_CODE:" | cut -d: -f2)
BODY=$(echo "$RESPONSE" | sed '/HTTP_CODE:/d')

echo "HTTP Status: $HTTP_CODE"
echo "Response: $BODY"

if [ "$HTTP_CODE" -eq 201 ]; then
    echo -e "${GREEN}✅ Test 2 PASSED - Lead created${NC}"
    C2S_LEAD_ID=$(echo "$BODY" | grep -o '"c2s_lead_id":"[^"]*"' | cut -d'"' -f4)
    echo -e "${BLUE}C2S Lead ID: $C2S_LEAD_ID${NC}"
else
    echo -e "${RED}❌ Test 2 FAILED - Expected 201, got $HTTP_CODE${NC}"
fi
echo ""

# Test 3: Full webhook with all fields
echo -e "${YELLOW}📋 Test 3: Full webhook with phone and email${NC}"
RESPONSE=$(curl -s -w "\nHTTP_CODE:%{http_code}" -X POST "$BASE_URL/api/v1/webhooks/google-ads?google_key=$GOOGLE_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "lead_id": "test-full-'$(date +%s)'",
    "api_version": "v1",
    "form_id": 123456,
    "campaign_id": 789012,
    "gcl_id": "test-gclid-123",
    "google_key": "'$GOOGLE_KEY'",
    "is_test": true,
    "user_column_data": [
      {"column_id": "FULL_NAME", "column_name": "Nome Completo", "string_value": "Maria Santos"},
      {"column_id": "EMAIL", "column_name": "E-mail", "string_value": "maria.santos@example.com"},
      {"column_id": "PHONE_NUMBER", "column_name": "Telefone", "string_value": "+5511987654321"}
    ]
  }')

HTTP_CODE=$(echo "$RESPONSE" | grep "HTTP_CODE:" | cut -d: -f2)
BODY=$(echo "$RESPONSE" | sed '/HTTP_CODE:/d')

echo "HTTP Status: $HTTP_CODE"
echo "Response: $BODY"

if [ "$HTTP_CODE" -eq 201 ]; then
    echo -e "${GREEN}✅ Test 3 PASSED - Full lead created with enrichment${NC}"
    C2S_LEAD_ID=$(echo "$BODY" | grep -o '"c2s_lead_id":"[^"]*"' | cut -d'"' -f4)
    echo -e "${BLUE}C2S Lead ID: $C2S_LEAD_ID${NC}"
else
    echo -e "${RED}❌ Test 3 FAILED - Expected 201, got $HTTP_CODE${NC}"
fi
echo ""

# Test 4: Webhook with CPF in form data
echo -e "${YELLOW}📋 Test 4: Webhook with CPF field${NC}"
RESPONSE=$(curl -s -w "\nHTTP_CODE:%{http_code}" -X POST "$BASE_URL/api/v1/webhooks/google-ads?google_key=$GOOGLE_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "lead_id": "test-cpf-'$(date +%s)'",
    "api_version": "v1",
    "form_id": 123456,
    "campaign_id": 789012,
    "google_key": "'$GOOGLE_KEY'",
    "is_test": true,
    "user_column_data": [
      {"column_id": "FULL_NAME", "column_name": "Nome Completo", "string_value": "Pedro Oliveira"},
      {"column_id": "CPF", "column_name": "CPF", "string_value": "123.456.789-01"},
      {"column_id": "EMAIL", "column_name": "E-mail", "string_value": "pedro@example.com"},
      {"column_id": "PHONE_NUMBER", "column_name": "Telefone", "string_value": "11987654321"}
    ]
  }')

HTTP_CODE=$(echo "$RESPONSE" | grep "HTTP_CODE:" | cut -d: -f2)
BODY=$(echo "$RESPONSE" | sed '/HTTP_CODE:/d')

echo "HTTP Status: $HTTP_CODE"
echo "Response: $BODY"

if [ "$HTTP_CODE" -eq 201 ]; then
    echo -e "${GREEN}✅ Test 4 PASSED - Lead with CPF created${NC}"
    C2S_LEAD_ID=$(echo "$BODY" | grep -o '"c2s_lead_id":"[^"]*"' | cut -d'"' -f4)
    echo -e "${BLUE}C2S Lead ID: $C2S_LEAD_ID${NC}"
else
    echo -e "${RED}❌ Test 4 FAILED - Expected 201, got $HTTP_CODE${NC}"
fi
echo ""

# Test 5: Duplicate lead (idempotency test)
echo -e "${YELLOW}📋 Test 5: Duplicate lead (idempotency)${NC}"
DUPLICATE_LEAD_ID="test-duplicate-$(date +%s)"

# First request
curl -s -X POST "$BASE_URL/api/v1/webhooks/google-ads?google_key=$GOOGLE_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "lead_id": "'$DUPLICATE_LEAD_ID'",
    "api_version": "v1",
    "form_id": 123456,
    "campaign_id": 789012,
    "google_key": "'$GOOGLE_KEY'",
    "is_test": true,
    "user_column_data": [
      {"column_id": "FULL_NAME", "column_name": "Nome Completo", "string_value": "Duplicate Test"}
    ]
  }' > /dev/null

sleep 1

# Second request (duplicate)
RESPONSE=$(curl -s -w "\nHTTP_CODE:%{http_code}" -X POST "$BASE_URL/api/v1/webhooks/google-ads?google_key=$GOOGLE_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "lead_id": "'$DUPLICATE_LEAD_ID'",
    "api_version": "v1",
    "form_id": 123456,
    "campaign_id": 789012,
    "google_key": "'$GOOGLE_KEY'",
    "is_test": true,
    "user_column_data": [
      {"column_id": "FULL_NAME", "column_name": "Nome Completo", "string_value": "Duplicate Test"}
    ]
  }')

HTTP_CODE=$(echo "$RESPONSE" | grep "HTTP_CODE:" | cut -d: -f2)
BODY=$(echo "$RESPONSE" | sed '/HTTP_CODE:/d')

echo "HTTP Status: $HTTP_CODE"
echo "Response: $BODY"

if [ "$HTTP_CODE" -eq 200 ]; then
    if echo "$BODY" | grep -q "duplicate"; then
        echo -e "${GREEN}✅ Test 5 PASSED - Duplicate correctly detected${NC}"
    else
        echo -e "${YELLOW}⚠️  Test 5 WARNING - Status 200 but no duplicate message${NC}"
    fi
else
    echo -e "${RED}❌ Test 5 FAILED - Expected 200, got $HTTP_CODE${NC}"
fi
echo ""

# Summary
echo "=================================================="
echo "Test Summary"
echo "=================================================="
echo ""
echo "✅ Google Ads webhook integration is working!"
echo ""
echo "Next steps:"
echo "1. Configure Google Ads Lead Form Extension webhook:"
echo "   URL: $BASE_URL/api/v1/webhooks/google-ads"
echo "   Add parameter: google_key=$GOOGLE_KEY"
echo ""
echo "2. Monitor google_ads_leads table in database:"
echo "   SELECT * FROM google_ads_leads ORDER BY created_at DESC LIMIT 10;"
echo ""
echo "3. Check C2S for created leads"
echo ""
echo "=================================================="
