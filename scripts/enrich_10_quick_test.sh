#!/bin/bash

C2S_TOKEN="${C2S_TOKEN:-4ecfcda34202be88a3f8ef70a79b097035621cca7dfe36b8b3}"
ENRICH_API="${ENRICH_API:-https://mbras-c2s.fly.dev}"

echo "=== Quick Test: Enriching 10 Leads ==="
echo ""

LEADS=$(curl -s "https://api.contact2sale.com/integration/leads?per_page=10&page=1" \
  -H "Authorization: Bearer $C2S_TOKEN")

LEAD_IDS=$(echo "$LEADS" | grep -o '"id":"[^"]*"' | head -10 | cut -d'"' -f4)
TOTAL=$(echo "$LEAD_IDS" | wc -l | tr -d ' ')

echo "Found $TOTAL leads"
echo ""

SUCCESS=0
FAILED=0

for LEAD_ID in $LEAD_IDS; do
  echo -n "$LEAD_ID... "
  
  START=$(date +%s)
  HTTP_CODE=$(curl -s -w "%{http_code}" -o /dev/null -X POST \
    "$ENRICH_API/api/v1/c2s/enrich/$LEAD_ID" \
    -H "Content-Type: application/json" \
    --max-time 90)
  END=$(date +%s)
  DURATION=$((END - START))
  
  if [ "$HTTP_CODE" = "200" ] || [ "$HTTP_CODE" = "409" ]; then
    echo "✓ Success (${DURATION}s)"
    SUCCESS=$((SUCCESS + 1))
  else
    echo "✗ Failed HTTP $HTTP_CODE (${DURATION}s)"
    FAILED=$((FAILED + 1))
  fi
  
  sleep 3
done

echo ""
echo "Results: $SUCCESS successful, $FAILED failed"
