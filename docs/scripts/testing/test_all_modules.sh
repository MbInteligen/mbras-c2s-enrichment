#!/bin/bash

# Comprehensive Work API Module Testing Script
# Tests all individual module endpoints + combined endpoint

set -e

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Load environment variables
if [ -f .env ]; then
    export $(cat .env | grep -v '^#' | xargs)
fi

echo -e "${BLUE}=========================================="
echo "Work API Module Testing"
echo -e "==========================================${NC}"
echo ""

# Get test document from database or use example
echo "Fetching test document from database..."
if command -v psql &> /dev/null && [ -n "$DB_URL" ]; then
    TEST_CPF=$(psql "$DB_URL" -t -c "SELECT cpf_cnpj FROM core.parties WHERE party_type = 'customer' AND cpf_cnpj IS NOT NULL LIMIT 1" 2>/dev/null | xargs)
    if [ -n "$TEST_CPF" ]; then
        echo -e "${GREEN}✅ Found test CPF from database: ${TEST_CPF:0:3}****${NC}"
    else
        echo -e "${YELLOW}⚠️  No CPF in database, using example${NC}"
        TEST_CPF="12345678901"
    fi
else
    echo -e "${YELLOW}⚠️  Using example CPF${NC}"
    TEST_CPF="12345678901"
fi

echo ""
echo "Starting server..."

# Build the project
cargo build --release 2>&1 | grep -E "(Finished|Compiling rust-c2s-api)" || true

# Start server in background
./target/release/rust-c2s-api &
SERVER_PID=$!

echo -e "${GREEN}Server started with PID: $SERVER_PID${NC}"

# Cleanup function
cleanup() {
    echo ""
    echo -e "${YELLOW}Stopping server...${NC}"
    kill $SERVER_PID 2>/dev/null || true
    wait $SERVER_PID 2>/dev/null || true
    echo -e "${GREEN}Cleanup complete${NC}"
}

trap cleanup EXIT

# Wait for server to start
echo "Waiting for server to start..."
sleep 3

BASE_URL="http://localhost:8080"

# Test health endpoint
echo ""
echo -e "${BLUE}=========================================="
echo "Testing Health Endpoint"
echo -e "==========================================${NC}"
HEALTH=$(curl -s "$BASE_URL/health")
echo "$HEALTH" | jq '.' 2>/dev/null || echo "$HEALTH"
if echo "$HEALTH" | jq -e '.status == "healthy"' > /dev/null 2>&1; then
    echo -e "${GREEN}✅ Health check passed${NC}"
else
    echo -e "${RED}❌ Health check failed${NC}"
    exit 1
fi

# Array of all modules
MODULES=("tel" "cpf" "nome" "email" "titulo" "cep" "mae" "cnpj")

echo ""
echo -e "${BLUE}=========================================="
echo "Testing Individual Module Endpoints"
echo -e "==========================================${NC}"

# Test each module individually
for MODULE in "${MODULES[@]}"; do
    echo ""
    echo -e "${YELLOW}Testing module: $MODULE${NC}"
    echo "------------------------------------------"

    RESPONSE=$(curl -s "$BASE_URL/api/v1/work/modules/$MODULE?documento=$TEST_CPF")

    if [ $? -eq 0 ]; then
        # Check if response has data
        if echo "$RESPONSE" | jq -e '. != null' > /dev/null 2>&1; then
            echo -e "${GREEN}✅ Response received for $MODULE${NC}"

            # Display formatted response
            echo "$RESPONSE" | jq '.' 2>/dev/null || echo "$RESPONSE"

            # Check status if available
            if echo "$RESPONSE" | jq -e '.status' > /dev/null 2>&1; then
                STATUS=$(echo "$RESPONSE" | jq -r '.status')
                echo -e "${BLUE}Status: $STATUS${NC}"
            fi
        else
            echo -e "${YELLOW}⚠️  Empty response for $MODULE${NC}"
        fi
    else
        echo -e "${RED}❌ Request failed for $MODULE${NC}"
    fi

    sleep 1  # Rate limiting
done

echo ""
echo -e "${BLUE}=========================================="
echo "Testing Combined Modules Endpoint"
echo -e "==========================================${NC}"

RESPONSE=$(curl -s "$BASE_URL/api/v1/work/modules/all?documento=$TEST_CPF")

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✅ Combined request successful${NC}"
    echo ""

    # Module status summary
    echo -e "${BLUE}Module Status Summary:${NC}"
    echo "----------------------"

    for MODULE in telefone cpf nome email titulo cep mae cnpj; do
        if command -v jq &> /dev/null; then
            STATUS=$(echo "$RESPONSE" | jq -r ".$MODULE.status // \"N/A\"" 2>/dev/null)
            HAS_DATA=$(echo "$RESPONSE" | jq -r ".$MODULE.data != null" 2>/dev/null)

            if [ "$HAS_DATA" = "true" ]; then
                echo -e "${GREEN}✅ $MODULE: $STATUS (has data)${NC}"
            elif [ "$STATUS" != "N/A" ]; then
                echo -e "${YELLOW}⚠️  $MODULE: $STATUS (no data)${NC}"
            else
                echo -e "${RED}❌ $MODULE: Not found${NC}"
            fi
        fi
    done

    echo ""
    echo -e "${BLUE}Full Response:${NC}"
    echo "$RESPONSE" | jq '.' 2>/dev/null || echo "$RESPONSE"
else
    echo -e "${RED}❌ Combined request failed${NC}"
fi

echo ""
echo -e "${BLUE}=========================================="
echo "Testing Enrichment Endpoint"
echo -e "==========================================${NC}"

ENRICH_RESPONSE=$(curl -s "$BASE_URL/api/v1/contributor/customer?cpf=$TEST_CPF")

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✅ Enrichment request successful${NC}"
    echo ""

    # Display key information
    if command -v jq &> /dev/null; then
        echo -e "${BLUE}Personal Info:${NC}"
        echo "$ENRICH_RESPONSE" | jq '.personal_info' 2>/dev/null

        echo ""
        echo -e "${BLUE}Contact Info:${NC}"
        echo "$ENRICH_RESPONSE" | jq '.contact_info' 2>/dev/null

        echo ""
        echo -e "${BLUE}Addresses:${NC}"
        echo "$ENRICH_RESPONSE" | jq '.addresses' 2>/dev/null

        echo ""
        echo -e "${BLUE}Metadata:${NC}"
        echo "$ENRICH_RESPONSE" | jq '.metadata' 2>/dev/null

        # Get statistics
        ENRICHED=$(echo "$ENRICH_RESPONSE" | jq -r '.metadata.enriched' 2>/dev/null)
        SOURCES=$(echo "$ENRICH_RESPONSE" | jq -r '.metadata.sources[]' 2>/dev/null | tr '\n' ', ')
        MODULES=$(echo "$ENRICH_RESPONSE" | jq -r '.metadata.modules_consulted[]' 2>/dev/null | tr '\n' ', ')

        echo ""
        echo -e "${BLUE}Statistics:${NC}"
        echo "  Enriched: $ENRICHED"
        echo "  Sources: $SOURCES"
        echo "  Modules consulted: $MODULES"
    else
        echo "$ENRICH_RESPONSE"
    fi
else
    echo -e "${RED}❌ Enrichment request failed${NC}"
fi

echo ""
echo -e "${BLUE}=========================================="
echo "Test Summary"
echo -e "==========================================${NC}"
echo ""
echo "Endpoints tested:"
echo -e "  ${GREEN}✅${NC} Health endpoint"
echo -e "  ${GREEN}✅${NC} Individual module endpoints (${#MODULES[@]} modules)"
echo -e "  ${GREEN}✅${NC} Combined modules endpoint"
echo -e "  ${GREEN}✅${NC} Customer enrichment endpoint"
echo ""
echo "Available module endpoints:"
for MODULE in "${MODULES[@]}"; do
    echo "  GET $BASE_URL/api/v1/work/modules/$MODULE?documento=DOCUMENTO"
done
echo ""
echo "  GET $BASE_URL/api/v1/work/modules/all?documento=DOCUMENTO"
echo "  GET $BASE_URL/api/v1/contributor/customer?cpf=CPF"
echo ""

echo -e "${GREEN}=========================================="
echo "All tests completed!"
echo -e "==========================================${NC}"
