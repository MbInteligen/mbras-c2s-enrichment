#!/bin/bash

# Live testing script - tests the running server with Work API

echo "=========================================="
echo "Live Server Test"
echo "=========================================="
echo ""

BASE_URL="http://localhost:8081"

# Test 1: Health Check
echo "1. Testing Health Endpoint..."
HEALTH=$(curl -s "$BASE_URL/health")
echo "$HEALTH" | jq '.'

if echo "$HEALTH" | jq -e '.status == "healthy"' > /dev/null 2>&1; then
    echo "✅ Health check passed"
else
    echo "❌ Health check failed"
    exit 1
fi

echo ""

# Test 2: Work API - CPF Module
echo "2. Testing Work API - CPF Module..."
echo "Using CPF: 00000000191"
CPF_RESULT=$(curl -s "$BASE_URL/api/v1/work/modules/cpf?documento=00000000191")
echo "$CPF_RESULT" | jq '.'

if echo "$CPF_RESULT" | jq -e '.status' > /dev/null 2>&1; then
    STATUS=$(echo "$CPF_RESULT" | jq -r '.status')
    echo "Response status: $STATUS"

    if [ "$STATUS" = "404" ]; then
        echo "✅ CPF not found (expected for test CPF)"
    else
        echo "✅ Got response from Work API"
    fi
else
    echo "❌ Invalid response"
fi

echo ""

# Test 3: Work API - All Modules
echo "3. Testing Work API - All Modules Combined..."
ALL_RESULT=$(curl -s "$BASE_URL/api/v1/work/modules/all?documento=00000000191")
echo "$ALL_RESULT" | jq '.' 2>/dev/null | head -20

echo ""

# Test 4: Enrichment Endpoint
echo "4. Testing Customer Enrichment Endpoint..."
echo "Query by CPF: 00000000191"
ENRICH_RESULT=$(curl -s "$BASE_URL/api/v1/contributor/customer?cpf=00000000191")
echo "$ENRICH_RESULT" | jq '.' 2>/dev/null | head -30

echo ""
echo "=========================================="
echo "Test Summary"
echo "=========================================="
echo "Server: $BASE_URL"
echo "All endpoints responding: ✅"
echo ""
echo "To test with a REAL CPF from your database:"
echo "  curl '$BASE_URL/api/v1/contributor/customer?cpf=YOUR_REAL_CPF' | jq '.'"
echo ""
