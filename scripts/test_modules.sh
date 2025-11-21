#!/bin/bash

# Direct Work API module testing script
# Tests all available modules and displays results

set -e

# Load environment variables
if [ -f .env ]; then
    export $(cat .env | grep -v '^#' | xargs)
fi

echo "=========================================="
echo "Work API Module Testing"
echo "=========================================="
echo "Token: ${WORK_API:0:10}..."
echo "Base URL: https://completa.workbuscas.com"
echo ""

# Get a test person from the database first
echo "Fetching a test person from database..."
TEST_QUERY="SELECT cpf_cnpj FROM core.parties WHERE party_type = 'customer' AND cpf_cnpj IS NOT NULL LIMIT 1"

# Using psql if available
if command -v psql &> /dev/null; then
    TEST_CPF=$(psql "$DB_URL" -t -c "$TEST_QUERY" | xargs)
    if [ -n "$TEST_CPF" ]; then
        echo "✅ Found test CPF: ${TEST_CPF:0:3}****"
    else
        echo "⚠️  No CPF found in database, using example"
        TEST_CPF="12345678901"
    fi
else
    echo "⚠️  psql not available, using example CPF"
    TEST_CPF="12345678901"
fi

echo ""
echo "=========================================="
echo "Testing Individual Modules"
echo "=========================================="

# List of all available modules based on the screenshot
MODULES=("tel" "cpf" "nome" "email" "titulo" "cep" "mae" "cnpj")

for MODULE in "${MODULES[@]}"; do
    echo ""
    echo "Testing module: $MODULE"
    echo "------------------------------------------"

    URL="https://completa.workbuscas.com/api?token=$WORK_API&modulo=$MODULE&consulta=$TEST_CPF"

    RESPONSE=$(curl -s "$URL")

    if [ $? -eq 0 ]; then
        echo "✅ Response received"
        echo "$RESPONSE" | jq '.' 2>/dev/null || echo "$RESPONSE"
    else
        echo "❌ Request failed"
    fi

    sleep 1  # Rate limiting
done

echo ""
echo "=========================================="
echo "Testing ALL Modules Combined"
echo "=========================================="

# All modules in one request (as shown in the screenshot)
ALL_MODULES="tel,cpf,nome,email,titulo,cep,mae,cnpj"
URL="https://completa.workbuscas.com/api?token=$WORK_API&modulo=$ALL_MODULES&consulta=$TEST_CPF"

echo "Fetching all modules for CPF: ${TEST_CPF:0:3}****"
echo ""

RESPONSE=$(curl -s "$URL")

if [ $? -eq 0 ]; then
    echo "✅ Combined request successful"
    echo ""

    # Parse and display each module's data
    if command -v jq &> /dev/null; then
        echo "Module Status Summary:"
        echo "----------------------"

        for MODULE in telefone cpf nome email titulo cep mae cnpj; do
            STATUS=$(echo "$RESPONSE" | jq -r ".$MODULE.status // \"N/A\"")
            HAS_DATA=$(echo "$RESPONSE" | jq -r ".$MODULE.data != null")

            if [ "$HAS_DATA" = "true" ]; then
                echo "✅ $MODULE: $STATUS (has data)"
            else
                echo "⚠️  $MODULE: $STATUS (no data)"
            fi
        done

        echo ""
        echo "Full Response:"
        echo "$RESPONSE" | jq '.'
    else
        echo "$RESPONSE"
    fi
else
    echo "❌ Combined request failed"
fi

echo ""
echo "=========================================="
echo "Test Summary"
echo "=========================================="
echo "Individual modules tested: ${#MODULES[@]}"
echo "Combined request: Completed"
echo ""
echo "To test the full API integration, run:"
echo "  cargo run"
echo "Then in another terminal:"
echo "  curl 'http://localhost:8080/api/v1/contributor/customer?cpf=$TEST_CPF' | jq '.'"
echo ""
