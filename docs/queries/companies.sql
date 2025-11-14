-- Company queries for Rust C2S API
-- Note: Using core.parties table with party_type='company'

-- name: get_company_by_id
-- Get company by ID
SELECT
    id,
    cpf_cnpj as cnpj,
    full_name as legal_name,
    normalized_name as normalized_legal_name,
    fantasy_name,
    normalized_fantasy_name,
    opening_date,
    registration_status_date,
    company_type as type,
    company_size as size,
    created_at,
    updated_at
FROM core.parties
WHERE id = $1 AND party_type = 'company';

-- name: get_companies_by_cnpj
-- Get companies by CNPJ
SELECT
    id,
    cpf_cnpj as cnpj,
    full_name as legal_name,
    normalized_name as normalized_legal_name,
    fantasy_name,
    normalized_fantasy_name,
    opening_date,
    registration_status_date,
    company_type as type,
    company_size as size,
    created_at,
    updated_at
FROM core.parties
WHERE cpf_cnpj = $1 AND party_type = 'company';

-- name: get_companies_by_name
-- Search companies by name pattern with pagination
SELECT
    id,
    cpf_cnpj as cnpj,
    full_name as legal_name,
    normalized_name as normalized_legal_name,
    fantasy_name,
    normalized_fantasy_name,
    opening_date,
    registration_status_date,
    company_type as type,
    company_size as size,
    created_at,
    updated_at
FROM core.parties
WHERE (normalized_name ILIKE $1
       OR normalized_fantasy_name ILIKE $1)
  AND party_type = 'company'
ORDER BY full_name
LIMIT $2
OFFSET $3;
