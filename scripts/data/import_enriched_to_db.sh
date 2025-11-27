#!/bin/bash

# Import enriched JSON files to PostgreSQL database

if [ -z "$DB_URL" ]; then
    echo "Error: DB_URL environment variable not set"
    echo "Please set it with: export DB_URL=postgresql://..."
    exit 1
fi

echo "=== Import Enriched Data to Database ==="
echo "Database: $DB_URL"
echo ""

success_count=0
fail_count=0
total=$(ls temp_enriched_*.json 2>/dev/null | wc -l | tr -d ' ')

if [ "$total" -eq 0 ]; then
    echo "No enriched JSON files found (temp_enriched_*.json)"
    exit 1
fi

echo "Found $total enriched files to import"
echo ""

for json_file in temp_enriched_*.json; do
    cpf=$(echo "$json_file" | sed 's/temp_enriched_//' | sed 's/.json//')
    echo "Processing $json_file (CPF: $cpf)"

    # Extract data from JSON
    nome=$(jq -r '.DadosBasicos.nome // ""' "$json_file")
    sexo=$(jq -r '.DadosBasicos.sexo // ""' "$json_file")
    data_nasc=$(jq -r '.DadosBasicos.dataNascimento // ""' "$json_file")
    mae=$(jq -r '.DadosBasicos.nomeMae // ""' "$json_file")
    pai=$(jq -r '.DadosBasicos.nomePai // ""' "$json_file")
    rg=$(jq -r '.DadosBasicos.rg // ""' "$json_file")

    # Escape single quotes for SQL
    nome=$(echo "$nome" | sed "s/'/''/g")
    mae=$(echo "$mae" | sed "s/'/''/g")
    pai=$(echo "$pai" | sed "s/'/''/g")

    # Insert into database
    psql "$DB_URL" <<EOF
        INSERT INTO core.parties (
            party_type, cpf_cnpj, full_name, sex, birth_date, mother_name, father_name, rg, enriched
        )
        VALUES (
            'customer',
            '$cpf',
            '$nome',
            NULLIF('$sexo', ''),
            NULLIF('$data_nasc', '')::date,
            NULLIF('$mae', ''),
            NULLIF('$pai', ''),
            NULLIF('$rg', ''),
            true
        )
        ON CONFLICT (cpf_cnpj) DO UPDATE SET
            full_name = EXCLUDED.full_name,
            sex = EXCLUDED.sex,
            birth_date = EXCLUDED.birth_date,
            mother_name = EXCLUDED.mother_name,
            father_name = EXCLUDED.father_name,
            rg = EXCLUDED.rg,
            enriched = true,
            updated_at = NOW()
        RETURNING id;
EOF

    if [ $? -eq 0 ]; then
        echo "  ✓ Stored in database"
        success_count=$((success_count + 1))

        # Now store emails
        emails=$(jq -r '.emails[]?.email // empty' "$json_file" 2>/dev/null | head -5)
        if [ -n "$emails" ]; then
            echo "$emails" | while read -r email; do
                if [ -n "$email" ]; then
                    psql "$DB_URL" -c "
                        WITH email_insert AS (
                            INSERT INTO app.emails (email) VALUES ('$email')
                            ON CONFLICT (email) DO UPDATE SET email = EXCLUDED.email
                            RETURNING id
                        ),
                        party_lookup AS (
                            SELECT id FROM core.parties WHERE cpf_cnpj = '$cpf' LIMIT 1
                        )
                        INSERT INTO core.party_emails (party_id, email_id)
                        SELECT p.id, e.id FROM party_lookup p, email_insert e
                        ON CONFLICT DO NOTHING;
                    " > /dev/null 2>&1
                fi
            done
        fi

        # Now store phones
        telefones=$(jq -r '.telefones[]?.telefone // empty' "$json_file" 2>/dev/null | head -5)
        if [ -n "$telefones" ]; then
            echo "$telefones" | while read -r telefone; do
                if [ -n "$telefone" ]; then
                    psql "$DB_URL" -c "
                        WITH phone_insert AS (
                            INSERT INTO app.phones (number) VALUES ('$telefone')
                            ON CONFLICT (number) DO UPDATE SET number = EXCLUDED.number
                            RETURNING id
                        ),
                        party_lookup AS (
                            SELECT id FROM core.parties WHERE cpf_cnpj = '$cpf' LIMIT 1
                        )
                        INSERT INTO core.party_phones (party_id, phone_id)
                        SELECT p.id, ph.id FROM party_lookup p, phone_insert ph
                        ON CONFLICT DO NOTHING;
                    " > /dev/null 2>&1
                fi
            done
        fi
    else
        echo "  ✗ Failed to store"
        fail_count=$((fail_count + 1))
    fi

    echo ""
done

echo "=== Import Complete ==="
echo "Total files: $total"
echo "✓ Success: $success_count"
echo "✗ Failed: $fail_count"
echo "Success rate: $(awk "BEGIN {printf \"%.1f\", ($success_count/$total)*100}")%"
