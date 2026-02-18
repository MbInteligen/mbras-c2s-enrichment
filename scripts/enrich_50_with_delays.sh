#!/bin/bash

C2S_TOKEN="${C2S_TOKEN:-4ecfcda34202be88a3f8ef70a79b097035621cca7dfe36b8b3}"
ENRICH_API="${ENRICH_API:-https://mbras-c2s.fly.dev}"

echo "=== Enriching 50 Leads from C2S (with 5s delays) ==="
echo ""

# Get 50 leads
LEADS=$(curl -s "https://api.contact2sale.com/integration/leads?per_page=50&page=1" \
  -H "Authorization: Bearer $C2S_TOKEN")

LEAD_IDS=$(echo "$LEADS" | grep -o '"id":"[^"]*"' | head -50 | cut -d'"' -f4)
TOTAL=$(echo "$LEAD_IDS" | wc -l | tr -d ' ')

echo "Found $TOTAL leads to process"
echo "Delay: 5 seconds between requests"
echo ""

SUCCESS=0
FAILED_404=0
FAILED_TIMEOUT=0
FAILED_RATE=0
FAILED_OTHER=0
SKIPPED=0

COUNT=0
for LEAD_ID in $LEAD_IDS; do
  COUNT=$((COUNT + 1))
  echo -n "[$COUNT/$TOTAL] $LEAD_ID... "
  
  TEMP_FILE=$(mktemp)
  HTTP_CODE=$(curl -s -w "%{http_code}" -o "$TEMP_FILE" -X POST \
    "$ENRICH_API/api/v1/c2s/enrich/$LEAD_ID" \
    -H "Content-Type: application/json" \
    --max-time 60)
  
  BODY=$(cat "$TEMP_FILE")
  rm -f "$TEMP_FILE"
  
  case "$HTTP_CODE" in
    200)
      echo "✓ Enriched & sent to C2S"
      SUCCESS=$((SUCCESS + 1))
      ;;
    409)
      echo "⊘ Already enriched"
      SKIPPED=$((SKIPPED + 1))
      ;;
    404)
      echo "✗ No CPF found (DBase/Mimir/Diretrix all failed)"
      FAILED_404=$((FAILED_404 + 1))
      ;;
    502|504)
      echo "✗ Timeout"
      FAILED_TIMEOUT=$((FAILED_TIMEOUT + 1))
      ;;
    429)
      echo "✗ Rate limited"
      FAILED_RATE=$((FAILED_RATE + 1))
      ;;
    *)
      ERROR=$(echo "$BODY" | grep -o '"error":"[^"]*"' | head -1 | cut -d'"' -f4)
      echo "✗ Error (HTTP $HTTP_CODE) - $ERROR"
      FAILED_OTHER=$((FAILED_OTHER + 1))
      ;;
  esac
  
  # 5 second delay to avoid rate limiting
  sleep 5
done

echo ""
echo "=== Summary ==="
echo "Total leads: $TOTAL"
echo "✓ Successfully enriched: $SUCCESS"
echo "⊘ Already enriched: $SKIPPED"
echo "✗ No CPF found: $FAILED_404"
echo "✗ Timeout: $FAILED_TIMEOUT"
echo "✗ Rate limited: $FAILED_RATE"
echo "✗ Other errors: $FAILED_OTHER"
echo ""
PROCESSED=$((SUCCESS + SKIPPED))
echo "Successfully processed: $PROCESSED/$TOTAL ($(awk "BEGIN {printf \"%.1f%%\", ($PROCESSED / $TOTAL) * 100}"))"
