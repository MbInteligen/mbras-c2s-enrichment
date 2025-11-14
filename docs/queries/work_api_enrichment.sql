-- SQL Queries for Work API Enrichment Data Storage
-- This file documents the SQL operations performed when storing Work API data
-- Source: src/db_storage.rs::EnrichmentStorage::store_enriched_person()

-- =============================================================================
-- 1. ENTITY MANAGEMENT (core.entities)
-- =============================================================================

-- Check if entity exists by CPF
SELECT entity_id
FROM core.entities
WHERE national_id = $1
LIMIT 1;

-- Update existing entity
UPDATE core.entities
SET is_enriched = true,
    enriched_at = now(),
    updated_at = now(),
    name = COALESCE(name, $2),
    canonical_name = COALESCE(canonical_name, $3)
WHERE national_id = $1;

-- Insert new entity if not exists
INSERT INTO core.entities (
    national_id,
    name,
    canonical_name,
    entity_type,
    is_enriched,
    enriched_at,
    data_source
)
VALUES (
    $1,  -- CPF
    $2,  -- nome
    $3,  -- canonical_name (uppercase)
    'person'::core.entity_type_enum,
    true,
    now(),
    'api'::core.data_source_enum
)
RETURNING entity_id;

-- =============================================================================
-- 2. PROFILE DATA (core.entity_profiles)
-- =============================================================================

-- Upsert profile information
INSERT INTO core.entity_profiles (
    entity_id,
    sex,                    -- First character of "M - MASCULINO" or "F - FEMININO"
    birth_date,             -- Converted from DD/MM/YYYY to YYYY-MM-DD
    nationality,
    marital_status,         -- estadoCivil from Work API
    education_level,        -- escolaridade from Work API
    metadata                -- JSON: mother_name, father_name, cor, municipio_nascimento, cns
)
VALUES ($1, $2, $3, $4, $5, $6, $7)
ON CONFLICT (entity_id) DO UPDATE
SET sex = COALESCE(entity_profiles.sex, EXCLUDED.sex),
    birth_date = COALESCE(entity_profiles.birth_date, EXCLUDED.birth_date),
    nationality = COALESCE(entity_profiles.nationality, EXCLUDED.nationality),
    marital_status = COALESCE(entity_profiles.marital_status, EXCLUDED.marital_status),
    education_level = COALESCE(entity_profiles.education_level, EXCLUDED.education_level),
    metadata = entity_profiles.metadata || EXCLUDED.metadata,
    updated_at = now();

-- =============================================================================
-- 3. FINANCIAL DATA (core.entity_financials)
-- =============================================================================

-- Check if financial record exists for current year
SELECT id
FROM core.entity_financials
WHERE entity_id = $1
  AND financial_year = $2
  AND financial_month IS NULL
LIMIT 1;

-- Update existing financial record
UPDATE core.entity_financials
SET reported_income = $3,    -- renda * 1.9 multiplier applied
    credit_score = $4,        -- scoreCSBA
    risk_score = $5,          -- Mapped from scoreCSBAFaixaRisco (0.1 to 0.9)
    metadata = $6,            -- JSON: poder_aquisitivo, mosaic
    updated_at = now()
WHERE entity_id = $1
  AND financial_year = $2
  AND financial_month IS NULL;

-- Insert new financial record
INSERT INTO core.entity_financials (
    entity_id,
    financial_year,          -- Current year
    reported_income,         -- renda * 1.9
    credit_score,            -- scoreCSBA
    risk_score,              -- Mapped risk level
    source,
    confidence,
    metadata                 -- JSON: poder_aquisitivo, serasaMosaic
)
VALUES (
    $1,
    $2,
    $3,
    $4,
    $5,
    'api'::core.data_source_enum,
    'high',
    $6
);

-- Risk Score Mapping:
-- "BAIXISSIMO RISCO" => 0.1
-- "BAIXO RISCO"      => 0.3
-- "MEDIO RISCO"      => 0.5
-- "ALTO RISCO"       => 0.7
-- "ALTISSIMO RISCO"  => 0.9

-- =============================================================================
-- 4. EMAILS (core.entity_emails)
-- =============================================================================

-- Insert email (ignore duplicates)
INSERT INTO core.entity_emails (
    entity_id,
    email,                   -- Lowercase normalized
    email_type,              -- Always 'personal'
    is_primary,              -- True for first email (idx == 0)
    is_verified,             -- True if qualidade == "BOM"
    metadata                 -- JSON: prioridade, qualidade, email_pessoal, blacklist
)
VALUES ($1, $2, 'personal', $3, $4, $5);

-- Note: Duplicate emails are silently ignored (no error handling)

-- =============================================================================
-- 5. PHONES (core.entity_phones)
-- =============================================================================

-- Insert phone (ignore duplicates)
INSERT INTO core.entity_phones (
    entity_id,
    phone,                   -- Raw phone number from Work API
    phone_type,              -- 'mobile' or 'landline' based on tipo field
    is_primary,              -- True for first phone (idx == 0)
    is_whatsapp,             -- True if whatsapp == "SIM"
    carrier,                 -- operadora from Work API
    metadata                 -- JSON: status
)
VALUES ($1, $2, $3, $4, $5, $6, $7);

-- Phone Type Mapping:
-- Contains "MÓVEL" or "MOVEL" => 'mobile'
-- Contains "RESIDENCIAL"      => 'landline'
-- Default                     => 'mobile'

-- =============================================================================
-- 6. ADDRESSES (app.addresses + core.entity_addresses)
-- =============================================================================

-- Step 1: Insert address into app.addresses
INSERT INTO app.addresses (
    street_type,             -- tipoLogradouro (e.g., "AV", "RUA")
    street,                  -- logradouro
    number,                  -- logradouroNumero
    complement,              -- complemento
    neighborhood,            -- bairro
    city,                    -- cidade
    state,                   -- uf
    zip_code                 -- cep
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
RETURNING id;

-- Step 2: Link address to entity
INSERT INTO core.entity_addresses (
    entity_id,
    address_id,              -- From previous INSERT RETURNING
    address_type,            -- Always 'residential'
    is_primary,              -- True for first address (idx == 0)
    is_current,              -- Always true
    data_source
)
VALUES (
    $1,
    $2,
    'residential',
    $3,
    true,
    'api'::core.data_source_enum
);

-- Note: Duplicate address links are silently ignored

-- =============================================================================
-- DATA TRANSFORMATIONS APPLIED
-- =============================================================================

-- 1. Income (renda):
--    - Replace comma with dot: "5000,00" => "5000.00"
--    - Parse to float
--    - Apply 1.9x multiplier
--    - Convert to BigDecimal

-- 2. Birth Date (dataNascimento):
--    - Input: "29/04/1975" (DD/MM/YYYY)
--    - Output: "1975-04-29" (YYYY-MM-DD)
--    - Split by "/" and reverse order

-- 3. Sex (sexo):
--    - Input: "M - MASCULINO" or "F - FEMININO"
--    - Output: "M" or "F" (first character only)

-- 4. Canonical Name:
--    - Convert to uppercase
--    - Stored in core.entities.canonical_name

-- 5. Email:
--    - Convert to lowercase
--    - Stored in core.entity_emails.email

-- =============================================================================
-- IMPORTANT NOTES
-- =============================================================================

-- 1. All operations use sequential queries (not CTEs) for sqlx compatibility
-- 2. Duplicate emails/phones/addresses are ignored (no ON CONFLICT handling)
-- 3. First item in array is marked as is_primary = true
-- 4. Financial data uses current year (no month specificity)
-- 5. All timestamps use now() for created_at/updated_at
-- 6. Data source is always 'api' for Work API enrichment
-- 7. Entity type is always 'person' (not 'company')
-- 8. Confidence level for financials is always 'high'
