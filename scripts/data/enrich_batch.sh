#!/bin/bash

# Batch enrichment script - calls our API endpoint to enrich and store CPFs

API_URL="${1:-https://mbras-c2s.fly.dev}"
CPF_FILE="${2:-cpf_list.txt}"

echo "=== Batch CPF Enrichment via API ==="
echo "API URL: $API_URL"
echo "CPF File: $CPF_FILE"
echo ""

if [ ! -f "$CPF_FILE" ]; then
    echo "Error: CPF file not found: $CPF_FILE"
    exit 1
fi

total=$(wc -l < "$CPF_FILE" | tr -d ' ')
current=0
success=0
failed=0

# Read CPF list and process each one
while IFS= read -r cpf; do
    current=$((current + 1))
    echo "[$current/$total] Processing CPF: $cpf"

    # Call our API to fetch enrichment data
    response=$(curl -s "${API_URL}/api/v1/work/modules/all?documento=${cpf}")

    # Check if API returned success
    status=$(echo "$response" | jq -r '.status // 0' 2>/dev/null)

    if [ "$status" = "200" ]; then
        echo "  ✓ Enriched successfully from Work API"

        # Save the enriched data to a temp file
        echo "$response" > "temp_enriched_${cpf}.json"

        # Now call a storage endpoint (we'll use psql directly for now)
        # Extract key data and insert into DB
        nome=$(echo "$response" | jq -r '.DadosBasicos.nome // ""')
        sexo=$(echo "$response" | jq -r '.DadosBasicos.sexo // ""')
        data_nasc=$(echo "$response" | jq -r '.DadosBasicos.dataNascimento // ""')
        mae=$(echo "$response" | jq -r '.DadosBasicos.nomeMae // ""')

        if [ -n "$DB_URL" ]; then
            # Insert into database using psql
            psql "$DB_URL" -c "
                INSERT INTO core.parties (party_type, cpf_cnpj, full_name, sex, birth_date, mother_name, enriched)
                VALUES ('customer', '$cpf', '$nome', '$sexo', '$data_nasc'::date, '$mae', true)
                ON CONFLICT (cpf_cnpj) DO UPDATE SET
                    full_name = EXCLUDED.full_name,
                    sex = EXCLUDED.sex,
                    birth_date = EXCLUDED.birth_date,
                    mother_name = EXCLUDED.mother_name,
                    enriched = true,
                    updated_at = NOW();
            " > /dev/null 2>&1

            if [ $? -eq 0 ]; then
                echo "  ✓ Stored in database"
                success=$((success + 1))
            else
                echo "  ✗ Failed to store in database"
                failed=$((failed + 1))
            fi
        else
            echo "  ⚠ DB_URL not set, skipping database storage"
            echo "  📁 Data saved to temp_enriched_${cpf}.json"
            success=$((success + 1))
        fi

    else
        echo "  ✗ Failed to enrich (status: $status)"
        echo "$response" | jq '.' 2>/dev/null || echo "$response"
        failed=$((failed + 1))
    fi

    # Rate limiting - wait 2 seconds between requests
    if [ $current -lt $total ]; then
        sleep 2
    fi

    echo ""
done < "$CPF_FILE"

echo "=== Batch Enrichment Complete ==="
echo "Total processed: $total"
echo "✓ Success: $success"
echo "✗ Failed: $failed"
echo "Success rate: $(awk "BEGIN {printf \"%.1f\", ($success/$total)*100}")%"
