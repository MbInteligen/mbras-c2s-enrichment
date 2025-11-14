-- Customer queries for Rust C2S API
-- Note: Using core.parties table with party_type='person'

-- name: get_customer_by_id
-- Get customer by ID
SELECT
    id,
    cpf_cnpj as cpf,
    full_name as name,
    normalized_name,
    sex::text,
    birth_date,
    mother_name,
    father_name,
    rg,
    enriched,
    created_at,
    updated_at
FROM core.parties
WHERE id = $1 AND party_type = 'person';

-- name: get_customer_by_cpf
-- Get customer by CPF
SELECT
    id,
    cpf_cnpj as cpf,
    full_name as name,
    normalized_name,
    sex::text,
    birth_date,
    mother_name,
    father_name,
    rg,
    enriched,
    created_at,
    updated_at
FROM core.parties
WHERE cpf_cnpj = $1 AND party_type = 'person';

-- name: get_customers_by_name
-- Search customers by name pattern with pagination
SELECT
    id,
    cpf_cnpj as cpf,
    full_name as name,
    normalized_name,
    sex::text,
    birth_date,
    mother_name,
    father_name,
    rg,
    enriched,
    created_at,
    updated_at
FROM core.parties
WHERE normalized_name ILIKE $1
  AND party_type = 'person'
ORDER BY full_name
LIMIT $2
OFFSET $3;

-- name: insert_customer
-- Insert new customer
INSERT INTO core.parties (
    party_type, cpf_cnpj, full_name, normalized_name,
    sex, birth_date, mother_name, father_name, rg, enriched
)
VALUES (
    'person', $1, $2, $3, $4, $5, $6, $7, $8, $9
)
RETURNING id, cpf_cnpj as cpf, full_name as name, created_at;

-- name: update_customer_enrichment
-- Update customer enrichment status
UPDATE core.parties
SET enriched = true, updated_at = now()
WHERE id = $1 AND party_type = 'person'
RETURNING id;
