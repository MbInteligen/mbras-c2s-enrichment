-- Migration 010: Backfill financial data into party_enrichments.normalized_data
-- Date: 2025-11-22
-- Purpose: Move 250 rows from core.entity_financials to Party Model

BEGIN;

-- ============================================================================
-- Step 1: Upsert enrichment rows for parties that have financials but no row
-- ============================================================================
INSERT INTO core.party_enrichments (
    enrichment_id,
    party_id,
    provider,
    raw_payload,
    normalized_data,
    quality_score,
    enriched_at,
    created_at
)
SELECT
    gen_random_uuid(),
    p.id AS party_id,
    'financials_legacy',
    '{}'::jsonb,
    jsonb_build_object('financials', jsonb_strip_nulls(to_jsonb(ef))),
    0.5,
    COALESCE(ef.updated_at, ef.created_at, now()),
    now()
FROM core.entity_financials ef
JOIN core.entities e ON ef.entity_id = e.entity_id
JOIN core.parties p ON p.cpf_cnpj = e.national_id
WHERE NOT EXISTS (
    SELECT 1 FROM core.party_enrichments pe WHERE pe.party_id = p.id
);

-- ============================================================================
-- Step 2: Update existing enrichments with financials payload
-- ============================================================================
UPDATE core.party_enrichments pe
SET normalized_data = pe.normalized_data || jsonb_build_object(
    'financials', jsonb_strip_nulls(to_jsonb(ef))
)
FROM core.entity_financials ef
JOIN core.entities e ON ef.entity_id = e.entity_id
JOIN core.parties p ON p.cpf_cnpj = e.national_id
WHERE pe.party_id = p.id;

-- ============================================================================
-- Step 3: Verification summary
-- ============================================================================
DO $$
DECLARE
    total_financials INTEGER;
    enriched_with_financials INTEGER;
BEGIN
    SELECT COUNT(*) INTO total_financials FROM core.entity_financials;
    SELECT COUNT(*) INTO enriched_with_financials
    FROM core.party_enrichments
    WHERE normalized_data ? 'financials';

    RAISE NOTICE 'Financial records in legacy: %', total_financials;
    RAISE NOTICE 'Party enrichments with financials: %', enriched_with_financials;
END $$;

COMMIT;
