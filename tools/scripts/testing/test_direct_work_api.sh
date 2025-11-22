#!/bin/bash

# Direct Work API testing without running the full server
# This tests the Work API directly to understand the response format

set -e

# Load environment
if [ -f .env ]; then
    export $(cat .env | grep -v '^#' | xargs)
fi

echo "=========================================="
echo "Direct Work API Testing"
echo "=========================================="
echo ""
echo "Token: ${WORK_API:0:10}..."
echo "Base URL: https://completa.workbuscas.com"
echo ""

# Use a test CPF
TEST_CPF="12345678901"

echo "Test Document: $TEST_CPF"
echo ""

MODULES=("tel" "cpf" "nome" "email" "titulo" "cep" "mae" "cnpj")

echo "=========================================="
echo "Testing Individual Modules"
echo "=========================================="

for MODULE in "${MODULES[@]}"; do
    echo ""
    echo "Module: $MODULE"
    echo "------------------------------------------"

    URL="https://completa.workbuscas.com/api?token=$WORK_API&modulo=$MODULE&consulta=$TEST_CPF"

    echo "Calling: $URL"
    RESPONSE=$(curl -s -w "\nHTTP_STATUS:%{http_code}" "$URL")

    HTTP_STATUS=$(echo "$RESPONSE" | grep "HTTP_STATUS:" | cut -d: -f2)
    BODY=$(echo "$RESPONSE" | sed -e 's/HTTP_STATUS:.*//g')

    echo "Status: $HTTP_STATUS"

    if [ "$HTTP_STATUS" = "200" ]; then
        echo "✅ Success"
        echo "$BODY" | jq '.' 2>/dev/null || echo "$BODY"
    else
        echo "❌ Failed"
        echo "$BODY"
    fi

    sleep 1
done

echo ""
echo "=========================================="
echo "Testing All Modules Combined"
echo "=========================================="

ALL_MODULES="tel,cpf,nome,email,titulo,cep,mae,cnpj"
URL="https://completa.workbuscas.com/api?token=$WORK_API&modulo=$ALL_MODULES&consulta=$TEST_CPF"

echo "Calling combined endpoint..."
echo "URL: $URL"
echo ""

RESPONSE=$(curl -s -w "\nHTTP_STATUS:%{http_code}" "$URL")

HTTP_STATUS=$(echo "$RESPONSE" | grep "HTTP_STATUS:" | cut -d: -f2)
BODY=$(echo "$RESPONSE" | sed -e 's/HTTP_STATUS:.*//g')

echo "Status: $HTTP_STATUS"
echo ""

if [ "$HTTP_STATUS" = "200" ]; then
    echo "✅ Success"
    echo ""
    echo "Full Response:"
    echo "$BODY" | jq '.' 2>/dev/null || echo "$BODY"

    echo ""
    echo "Module Summary:"
    echo "---------------"
    for MODULE in telefone cpf nome email titulo cep mae cnpj; do
        if command -v jq &> /dev/null; then
            STATUS=$(echo "$BODY" | jq -r ".$MODULE.status // \"N/A\"" 2>/dev/null)
            HAS_DATA=$(echo "$BODY" | jq ".$MODULE.data != null" 2>/dev/null)

            if [ "$HAS_DATA" = "true" ]; then
                echo "✅ $MODULE: $STATUS"
            else
                echo "⚠️  $MODULE: $STATUS"
            fi
        fi
    done
else
    echo "❌ Failed"
    echo "$BODY"
fi

echo ""
echo "=========================================="
echo "Test Complete"
echo "=========================================="
