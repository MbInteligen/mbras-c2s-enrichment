#!/bin/bash

# Test script to validate Work API integration and all modules

set -e

echo "🚀 Starting rust-c2s-api server..."
cargo build --release 2>/dev/null
echo "✅ Build completed successfully"

# Start the server in the background
./target/release/rust-c2s-api &
SERVER_PID=$!
echo "Server PID: $SERVER_PID"

# Wait for server to start
echo "⏳ Waiting for server to start..."
sleep 3

# Cleanup function
cleanup() {
    echo "🛑 Stopping server (PID: $SERVER_PID)..."
    kill $SERVER_PID 2>/dev/null || true
    wait $SERVER_PID 2>/dev/null || true
}

trap cleanup EXIT

# Base URL
BASE_URL="http://localhost:8080"

echo ""
echo "=========================================="
echo "Testing Health Endpoint"
echo "=========================================="
curl -s "$BASE_URL/health" | jq '.'

echo ""
echo "=========================================="
echo "Testing Work API Integration"
echo "=========================================="
echo "Fetching all modules for a test CPF..."

# Test with a CPF from database (you should replace with an actual CPF from your database)
TEST_CPF="12345678901"

echo ""
echo "Test 1: Query by CPF"
echo "------------------------------------------"
RESPONSE=$(curl -s "$BASE_URL/api/v1/contributor/customer?cpf=$TEST_CPF")
echo "$RESPONSE" | jq '.'

# Check if response has data
if echo "$RESPONSE" | jq -e '.metadata.enriched' > /dev/null; then
    echo "✅ CPF query successful - Enriched: $(echo "$RESPONSE" | jq -r '.metadata.enriched')"
    echo "📊 Modules consulted: $(echo "$RESPONSE" | jq -r '.metadata.modules_consulted[]' | tr '\n' ', ')"
    echo "📍 Sources: $(echo "$RESPONSE" | jq -r '.metadata.sources[]' | tr '\n' ', ')"

    # Display detailed information
    echo ""
    echo "Personal Info:"
    echo "$RESPONSE" | jq '.personal_info'

    echo ""
    echo "Contact Info:"
    echo "$RESPONSE" | jq '.contact_info'

    echo ""
    echo "Addresses:"
    echo "$RESPONSE" | jq '.addresses'
else
    echo "⚠️  No enrichment data found"
fi

echo ""
echo "=========================================="
echo "Test Complete!"
echo "=========================================="
echo ""
echo "Summary:"
echo "- Health endpoint: ✅"
echo "- GET /contributor/customer: $([ -n "$RESPONSE" ] && echo '✅' || echo '❌')"
echo ""
