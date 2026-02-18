#!/bin/bash

# Script to fetch last 50 leads from C2S and enrich them
# Uses the C2S API to get leads and the enrichment API to process them

C2S_TOKEN="${C2S_TOKEN:-4ecfcda34202be88a3f8ef70a79b097035621cca7dfe36b8b3}"
C2S_API="https://api.contact2sale.com"
ENRICH_API="${ENRICH_API:-https://mbras-c2s.fly.dev}"

echo "=== Fetching Last 50 Leads from C2S ==="
echo "C2S API: $C2S_API"
echo "Enrichment API: $ENRICH_API"
echo ""

# Fetch leads from C2S (last 50)
echo "Fetching leads..."
LEADS=$(curl -s -X GET "$C2S_API/integration/leads?per_page=50&page=1" \
  -H "Authorization: Bearer $C2S_TOKEN" \
  -H "Accept: application/json")

# Check if request was successful
if [ $? -ne 0 ]; then
  echo "❌ Failed to fetch leads from C2S"
  exit 1
fi

# Extract lead IDs
LEAD_IDS=$(echo "$LEADS" | jq -r '.data[].id' 2>/dev/null)

if [ -z "$LEAD_IDS" ]; then
  echo "❌ No leads found or failed to parse response"
  echo "Response: $LEADS" | head -20
  exit 1
fi

# Count leads
LEAD_COUNT=$(echo "$LEAD_IDS" | wc -l | tr -d ' ')
echo "✓ Found $LEAD_COUNT leads"
echo ""

# Enrich each lead
SUCCESS=0
FAILED=0
SKIPPED=0

echo "=== Starting Enrichment ==="
echo ""

for LEAD_ID in $LEAD_IDS; do
  echo -n "Processing lead $LEAD_ID... "
  
  # Call enrichment endpoint and capture response
  TEMP_FILE=$(mktemp)
  HTTP_CODE=$(curl -s -w "%{http_code}" -o "$TEMP_FILE" -X POST \
    "$ENRICH_API/api/v1/c2s/enrich/$LEAD_ID" \
    -H "Content-Type: application/json")
  
  BODY=$(cat "$TEMP_FILE")
  rm -f "$TEMP_FILE"
  
  if [ "$HTTP_CODE" = "200" ]; then
    echo "✓ Success"
    SUCCESS=$((SUCCESS + 1))
  elif [ "$HTTP_CODE" = "409" ]; then
    echo "⊘ Skipped (already processed)"
    SKIPPED=$((SKIPPED + 1))
  else
    echo "✗ Failed (HTTP $HTTP_CODE)"
    ERROR_MSG=$(echo "$BODY" | jq -r '.error // .message // "Unknown error"' 2>/dev/null)
    if [ -z "$ERROR_MSG" ] || [ "$ERROR_MSG" = "null" ]; then
      ERROR_MSG="$BODY"
    fi
    echo "  Error: $ERROR_MSG"
    FAILED=$((FAILED + 1))
  fi
  
  # Rate limiting: 3 second delay between requests
  sleep 3
done

echo ""
echo "=== Enrichment Summary ==="
echo "Total leads: $LEAD_COUNT"
echo "✓ Successfully enriched: $SUCCESS"
echo "⊘ Skipped (duplicates): $SKIPPED"
echo "✗ Failed: $FAILED"
echo ""

if [ "$LEAD_COUNT" -gt 0 ]; then
  SUCCESS_RATE=$(awk "BEGIN {printf \"%.1f%%\", ($SUCCESS / $LEAD_COUNT) * 100}")
  echo "Success rate: $SUCCESS_RATE"
fi
