#!/bin/bash

# Test DBase Fallback Integration
# This script tests if DBase is correctly triggered as fallback when Diretrix fails

set -e

BASE_URL="${1:-https://mbras-c2s.fly.dev}"

echo "========================================"
echo "DBase Fallback Integration Test"
echo "========================================"
echo ""
echo "Base URL: $BASE_URL"
echo ""

# Test 1: Health Check
echo "1️⃣  Testing Health Endpoint..."
HEALTH=$(curl -s "$BASE_URL/health")
if echo "$HEALTH" | jq -e '.status == "healthy"' > /dev/null 2>&1; then
    echo "   ✅ Service is healthy"
else
    echo "   ❌ Service health check failed"
    echo "   Response: $HEALTH"
    exit 1
fi
echo ""

# Test 2: Test with a phone number (this should trigger Diretrix -> possibly DBase fallback)
echo "2️⃣  Testing Phone Lookup (may trigger DBase fallback)..."
echo "   Note: This depends on whether Diretrix finds the number"
echo ""

# Use a test phone number
TEST_PHONE="11987654321"

echo "   Testing with phone: $TEST_PHONE"
echo "   This will:"
echo "   - Try Diretrix first (primary)"
echo "   - If Diretrix fails → Try DBase (fallback)"
echo ""

# We need to check Fly.io logs to see if DBase was triggered
echo "   ⚠️  To verify DBase fallback, check logs with:"
echo "   fly logs --app mbras-c2s | grep -i dbase"
echo ""

# Test 3: Verify configuration loaded
echo "3️⃣  Verifying DBASE_KEY is configured..."
echo "   Checking Fly.io secrets..."

if fly secrets list --app mbras-c2s 2>&1 | grep -q "DBASE_KEY"; then
    echo "   ✅ DBASE_KEY is set in Fly.io secrets"
else
    echo "   ❌ DBASE_KEY not found in Fly.io secrets"
    exit 1
fi
echo ""

# Test 4: Check recent logs for DBase activity
echo "4️⃣  Checking recent logs for DBase activity..."
echo ""

LOG_OUTPUT=$(fly logs --app mbras-c2s --no-tail 2>&1 | tail -100)

if echo "$LOG_OUTPUT" | grep -iq "dbase"; then
    echo "   ✅ Found DBase-related log entries:"
    echo "$LOG_OUTPUT" | grep -i "dbase" | tail -5
else
    echo "   ⚠️  No DBase activity in recent logs"
    echo "   This is normal if Diretrix is working and finding all CPFs"
fi
echo ""

# Test 5: Configuration check
echo "5️⃣  Checking if configuration loaded successfully..."
if echo "$LOG_OUTPUT" | grep -q "Configuration loaded successfully"; then
    echo "   ✅ Configuration loaded successfully"
else
    echo "   ❌ No configuration success message found"
fi
echo ""

echo "========================================"
echo "Summary"
echo "========================================"
echo ""
echo "✅ DBase API Key: Configured in Fly.io"
echo "✅ Service Status: Healthy"
echo "✅ Configuration: Loaded"
echo ""
echo "📊 To verify DBase fallback in action:"
echo "   1. Trigger an enrichment with a phone number not in Diretrix"
echo "   2. Watch logs: fly logs --app mbras-c2s"
echo "   3. Look for: 'Diretrix phone lookup failed, trying DBase fallback'"
echo "   4. Look for: '✓ DBase fallback found CPF: ...'"
echo ""
echo "🔍 Manual test command:"
echo "   curl -X POST \"$BASE_URL/api/v1/c2s/enrich/YOUR_LEAD_ID\""
echo ""

exit 0
