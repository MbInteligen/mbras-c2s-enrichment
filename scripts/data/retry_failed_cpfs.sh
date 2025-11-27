#!/bin/bash

# Retry failed CPFs with better rate limiting

API_URL="${1:-https://mbras-c2s.fly.dev}"
FAILED_FILE="${2:-failed_cpfs.txt}"
RETRY_DELAY="${3:-5}"  # Default 5 seconds between requests

if [ ! -f "$FAILED_FILE" ]; then
    echo "Error: Failed CPFs file not found: $FAILED_FILE"
    exit 1
fi

echo "=== Retry Failed CPFs Enrichment ==="
echo "API URL: $API_URL"
echo "Failed file: $FAILED_FILE"
echo "Delay between requests: ${RETRY_DELAY}s"
echo ""

total=$(wc -l < "$FAILED_FILE" | tr -d ' ')
current=0
success=0
failed=0

echo "Found $total failed CPFs to retry"
echo ""

while IFS= read -r cpf; do
    current=$((current + 1))
    echo "[$current/$total] Retrying CPF: $cpf"

    # Call API with longer timeout
    response=$(curl -s --max-time 60 "${API_URL}/api/v1/work/modules/all?documento=${cpf}")

    # Check if API returned success
    status=$(echo "$response" | jq -r '.status // 0' 2>/dev/null)

    if [ "$status" = "200" ]; then
        echo "  ✓ Enriched successfully"
        echo "$response" > "temp_enriched_${cpf}.json"
        success=$((success + 1))
    else
        echo "  ✗ Failed to enrich (status: $status)"
        echo "$response" | jq '.' 2>/dev/null | head -5 || echo "$response"
        failed=$((failed + 1))
    fi

    # Rate limiting - longer delay between requests
    if [ $current -lt $total ]; then
        echo "  ⏱  Waiting ${RETRY_DELAY}s before next request..."
        sleep $RETRY_DELAY
    fi

    echo ""
done < "$FAILED_FILE"

echo "=== Retry Complete ==="
echo "Total retried: $total"
echo "✓ Success: $success"
echo "✗ Failed: $failed"
echo "Success rate: $(awk "BEGIN {printf \"%.1f\", ($success/$total)*100}")%"
