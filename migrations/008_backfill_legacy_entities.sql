-- Migration: Backfill Legacy Entities to Party Model (V4 aligned to current schema)
-- Date: 2025-11-22
-- Purpose: Populate normalized party tables from core.entities. Idempotent and safe to rerun.
--
-- Key adjustments to fit this DB:
-- - core.parties uses id (PK) and party_type TEXT; no status/confidence/canonical_name/metadata columns.
-- - No core.entity_company_profiles table; company data pulled from core.entities + core.entity_profiles.
-- - Uses existing enums on target tables (contact_type_enum, ownership_type_enum, party_relationship_enum).
-- - Guards for uniques/PKs added only if missing.
-- - Anti-joins / ON CONFLICT to avoid duplicates on rerun.

CREATE EXTENSION IF NOT EXISTS pgcrypto;

BEGIN;

-- -----------------------------------------------------------------------------
-- Constraint guards (idempotent)
-- -----------------------------------------------------------------------------
DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint c
    JOIN pg_class t ON t.oid = c.conrelid
    JOIN pg_namespace n ON n.oid = t.relnamespace
    WHERE c.conname = 'uq_party_contact_unique'
      AND n.nspname = 'core'
      AND t.relname = 'party_contacts'
  ) THEN
    ALTER TABLE core.party_contacts
      ADD CONSTRAINT uq_party_contact_unique
      UNIQUE (party_id, contact_type, value);
  END IF;
END $$;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint c
    JOIN pg_class t ON t.oid = c.conrelid
    JOIN pg_namespace n ON n.oid = t.relnamespace
    WHERE c.conname = 'uq_party_enrichment_one'
      AND n.nspname = 'core'
      AND t.relname = 'party_enrichments'
  ) THEN
    ALTER TABLE core.party_enrichments
      ADD CONSTRAINT uq_party_enrichment_one UNIQUE (party_id);
  END IF;
END $$;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint c
    JOIN pg_class t ON t.oid = c.conrelid
    JOIN pg_namespace n ON n.oid = t.relnamespace
    WHERE c.conname = 'uq_party_rel_unique'
      AND n.nspname = 'core'
      AND t.relname = 'party_relationships'
  ) THEN
    ALTER TABLE core.party_relationships
      ADD CONSTRAINT uq_party_rel_unique
      UNIQUE (source_party_id, target_party_id, relationship_type, start_date);
  END IF;
END $$;

-- -----------------------------------------------------------------------------
-- 1) core.parties (uses existing columns)
-- -----------------------------------------------------------------------------
INSERT INTO core.parties (
  id,
  party_type,
  cpf_cnpj,
  full_name,
  normalized_name,
  enriched,
  created_at,
  updated_at,
  birth_date,
  sex,
  mother_name,
  opening_date,
  company_type,
  company_size
)
SELECT
  e.entity_id,
  e.entity_type::text,
  e.national_id,
  e.name,
  e.canonical_name,
  COALESCE(e.is_enriched, false),
  COALESCE(e.created_at, now()),
  COALESCE(e.updated_at, e.created_at, now()),
  p.birth_date,
  p.sex,
  NULL::text AS mother_name,
  p.opening_date,
  p.company_type,
  p.company_size
FROM core.entities e
LEFT JOIN core.entity_profiles p ON p.entity_id = e.entity_id
WHERE e.is_active = true
  AND e.entity_type IN ('person','company')
ON CONFLICT (id) DO UPDATE
SET
  party_type      = EXCLUDED.party_type,
  cpf_cnpj        = COALESCE(EXCLUDED.cpf_cnpj, core.parties.cpf_cnpj),
  full_name       = COALESCE(EXCLUDED.full_name, core.parties.full_name),
  normalized_name = COALESCE(EXCLUDED.normalized_name, core.parties.normalized_name),
  enriched        = EXCLUDED.enriched,
  birth_date      = COALESCE(EXCLUDED.birth_date, core.parties.birth_date),
  sex             = COALESCE(EXCLUDED.sex, core.parties.sex),
  mother_name     = COALESCE(EXCLUDED.mother_name, core.parties.mother_name),
  opening_date    = COALESCE(EXCLUDED.opening_date, core.parties.opening_date),
  company_type    = COALESCE(EXCLUDED.company_type, core.parties.company_type),
  company_size    = COALESCE(EXCLUDED.company_size, core.parties.company_size),
  updated_at      = EXCLUDED.updated_at;

-- -----------------------------------------------------------------------------
-- 2) core.people (PF)
-- -----------------------------------------------------------------------------
INSERT INTO core.people (
  party_id,
  full_name,
  mothers_name,
  birth_date,
  sex,
  marital_status,
  document_cpf,
  created_at,
  updated_at
)
SELECT
  e.entity_id,
  COALESCE(p.metadata->>'full_name', e.name),
  p.metadata->>'mothers_name',
  p.birth_date,
  p.sex,
  p.marital_status,
  e.national_id,
  COALESCE(p.created_at, e.created_at, now()),
  COALESCE(p.updated_at, e.updated_at, now())
FROM core.entities e
LEFT JOIN core.entity_profiles p ON p.entity_id = e.entity_id
WHERE e.is_active = true
  AND e.entity_type = 'person'
ON CONFLICT (party_id) DO UPDATE
SET
  full_name      = EXCLUDED.full_name,
  mothers_name   = COALESCE(EXCLUDED.mothers_name, core.people.mothers_name),
  birth_date     = COALESCE(EXCLUDED.birth_date, core.people.birth_date),
  sex            = COALESCE(EXCLUDED.sex, core.people.sex),
  marital_status = COALESCE(EXCLUDED.marital_status, core.people.marital_status),
  document_cpf   = COALESCE(EXCLUDED.document_cpf, core.people.document_cpf),
  updated_at     = EXCLUDED.updated_at;

-- -----------------------------------------------------------------------------
-- 3) core.companies (PJ) — from entities + entity_profiles
-- -----------------------------------------------------------------------------
INSERT INTO core.companies (
  party_id,
  legal_name,
  trade_name,
  cnpj,
  company_size,
  industry,
  foundation_date,
  created_at,
  updated_at
)
SELECT
  e.entity_id,
  COALESCE(e.metadata->>'legal_name', e.name),
  e.metadata->>'trade_name',
  e.national_id,
  p.company_size,
  p.main_activity,
  p.opening_date,
  COALESCE(e.created_at, now()),
  COALESCE(e.updated_at, e.created_at, now())
FROM core.entities e
LEFT JOIN core.entity_profiles p ON p.entity_id = e.entity_id
WHERE e.is_active = true
  AND e.entity_type = 'company'
ON CONFLICT (party_id) DO UPDATE
SET
  legal_name      = EXCLUDED.legal_name,
  trade_name      = COALESCE(EXCLUDED.trade_name, core.companies.trade_name),
  cnpj            = COALESCE(EXCLUDED.cnpj, core.companies.cnpj),
  company_size    = COALESCE(EXCLUDED.company_size, core.companies.company_size),
  industry        = COALESCE(EXCLUDED.industry, core.companies.industry),
  foundation_date = COALESCE(EXCLUDED.foundation_date, core.companies.foundation_date),
  updated_at      = EXCLUDED.updated_at;

-- -----------------------------------------------------------------------------
-- 4) core.party_contacts (EMAILS) – dedup via unique constraint
-- -----------------------------------------------------------------------------
INSERT INTO core.party_contacts (
  contact_id,
  party_id,
  contact_type,
  value,
  is_primary,
  is_verified,
  is_whatsapp,
  source,
  confidence,
  valid_from,
  valid_to,
  created_at,
  updated_at
)
SELECT
  gen_random_uuid(),
  ee.entity_id,
  'email'::core.contact_type_enum,
  lower(trim(ee.email)),
  COALESCE(ee.is_primary, false),
  COALESCE(ee.is_verified, false),
  false,
  COALESCE(ee.metadata->>'source', 'legacy'),
  COALESCE((ee.metadata->>'confidence')::numeric, 0),
  COALESCE(ee.created_at, now()),
  NULL,
  COALESCE(ee.created_at, now()),
  COALESCE(ee.updated_at, ee.created_at, now())
FROM core.entity_emails ee
JOIN core.entities e ON e.entity_id = ee.entity_id
WHERE e.is_active = true
  AND e.entity_type IN ('person','company')
  AND ee.email IS NOT NULL
ON CONFLICT ON CONSTRAINT uq_party_contact_unique DO NOTHING;

-- -----------------------------------------------------------------------------
-- 5) core.party_contacts (PHONES/WHATSAPP) – normalize digits, dedup
-- -----------------------------------------------------------------------------
INSERT INTO core.party_contacts (
  contact_id,
  party_id,
  contact_type,
  value,
  is_primary,
  is_verified,
  is_whatsapp,
  source,
  confidence,
  valid_from,
  valid_to,
  created_at,
  updated_at
)
SELECT
  gen_random_uuid(),
  ep.entity_id,
  CASE WHEN COALESCE(ep.is_whatsapp,false) THEN 'whatsapp'::core.contact_type_enum ELSE 'phone'::core.contact_type_enum END,
  regexp_replace(trim(ep.phone), '\D', '', 'g'),
  COALESCE(ep.is_primary, false),
  COALESCE(ep.is_verified, true),
  COALESCE(ep.is_whatsapp, false),
  COALESCE(ep.metadata->>'source', 'legacy'),
  COALESCE((ep.metadata->>'confidence')::numeric, 0),
  COALESCE(ep.created_at, now()),
  NULL,
  COALESCE(ep.created_at, now()),
  COALESCE(ep.updated_at, ep.created_at, now())
FROM core.entity_phones ep
JOIN core.entities e ON e.entity_id = ep.entity_id
WHERE e.is_active = true
  AND e.entity_type IN ('person','company')
  AND ep.phone IS NOT NULL
  AND length(regexp_replace(trim(ep.phone), '\D', '', 'g')) >= 8
ON CONFLICT ON CONSTRAINT uq_party_contact_unique DO NOTHING;

-- -----------------------------------------------------------------------------
-- 6) core.party_enrichments – one row per party
-- -----------------------------------------------------------------------------
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
  e.entity_id,
  COALESCE(e.metadata->>'provider','legacy'),
  COALESCE(e.metadata, '{}'::jsonb),
  '{}'::jsonb,
  CASE e.confidence
    WHEN 'very_low'::core.confidence_level THEN 0.1
    WHEN 'low'::core.confidence_level THEN 0.3
    WHEN 'medium'::core.confidence_level THEN 0.5
    WHEN 'high'::core.confidence_level THEN 0.8
    WHEN 'very_high'::core.confidence_level THEN 0.9
    WHEN 'verified'::core.confidence_level THEN 1.0
    ELSE 0.0
  END,
  COALESCE(e.enriched_at, e.updated_at, now()),
  COALESCE(e.enriched_at, now())
FROM core.entities e
WHERE e.is_active = true
  AND e.entity_type IN ('person','company')
  AND COALESCE(e.is_enriched,false) = true
  AND e.metadata IS NOT NULL
ON CONFLICT (party_id) DO UPDATE
SET
  provider      = EXCLUDED.provider,
  raw_payload   = EXCLUDED.raw_payload,
  quality_score = GREATEST(core.party_enrichments.quality_score, EXCLUDED.quality_score),
  enriched_at   = EXCLUDED.enriched_at;

-- -----------------------------------------------------------------------------
-- 7) core.ownerships – temporal property/party link
-- -----------------------------------------------------------------------------
INSERT INTO core.ownerships (
  property_id,
  party_id,
  ownership_type,
  ownership_percent,
  start_date,
  end_date,
  is_current,
  source,
  confidence,
  created_at,
  updated_at
)
SELECT
  po.property_id,
  po.entity_id,
  COALESCE(po.ownership_type::core.ownership_type_enum, 'full'),
  LEAST(GREATEST(COALESCE(po.ownership_percentage, 1.0),0),1),
  COALESCE(po.start_date, CURRENT_DATE),
  po.end_date,
  COALESCE(po.is_current, (po.end_date IS NULL OR po.end_date > CURRENT_DATE)),
  po.data_source,
  COALESCE(po.confidence, 0),
  COALESCE(po.created_at, now()),
  COALESCE(po.updated_at, po.created_at, now())
FROM core.property_ownerships po
JOIN core.entities e ON e.entity_id = po.entity_id
WHERE e.is_active = true
  AND e.entity_type IN ('person','company')
  AND po.property_id IS NOT NULL
  AND NOT EXISTS (
    SELECT 1 FROM core.ownerships o
    WHERE o.property_id = po.property_id
      AND o.party_id    = po.entity_id
      AND o.start_date  = COALESCE(po.start_date, CURRENT_DATE)
  );

-- -----------------------------------------------------------------------------
-- 8) core.party_relationships – graph from entity_relationships
-- -----------------------------------------------------------------------------
INSERT INTO core.party_relationships (
  relationship_id,
  source_party_id,
  target_party_id,
  relationship_type,
  start_date,
  end_date,
  is_current,
  metadata,
  confidence,
  created_at,
  updated_at
)
SELECT
  gen_random_uuid(),
  er.source_entity_id,
  er.target_entity_id,
  CASE er.relationship_type
    WHEN 'parent' THEN 'parent'::core.party_relationship_enum
    WHEN 'child' THEN 'child'::core.party_relationship_enum
    WHEN 'spouse' THEN 'spouse'::core.party_relationship_enum
    WHEN 'partner' THEN 'partner'::core.party_relationship_enum
    WHEN 'associate' THEN 'associate'::core.party_relationship_enum
    ELSE 'other'::core.party_relationship_enum
  END,
  COALESCE(er.valid_from, CURRENT_DATE),
  er.valid_until,
  COALESCE(er.is_current, (er.valid_until IS NULL OR er.valid_until > CURRENT_DATE)),
  COALESCE(er.metadata, '{}'::jsonb),
  COALESCE(er.confidence, 0),
  COALESCE(er.created_at, now()),
  COALESCE(er.updated_at, er.created_at, now())
FROM core.entity_relationships er
JOIN core.entities s ON s.entity_id = er.source_entity_id
JOIN core.entities t ON t.entity_id = er.target_entity_id
WHERE s.is_active = true AND t.is_active = true
  AND s.entity_type IN ('person','company')
  AND t.entity_type IN ('person','company')
  AND er.source_entity_id <> er.target_entity_id
  AND NOT EXISTS (
    SELECT 1 FROM core.party_relationships pr
    WHERE pr.source_party_id = er.source_entity_id
      AND pr.target_party_id = er.target_entity_id
      AND pr.relationship_type = CASE er.relationship_type
        WHEN 'parent' THEN 'parent'::core.party_relationship_enum
        WHEN 'child' THEN 'child'::core.party_relationship_enum
        WHEN 'spouse' THEN 'spouse'::core.party_relationship_enum
        WHEN 'partner' THEN 'partner'::core.party_relationship_enum
        WHEN 'associate' THEN 'associate'::core.party_relationship_enum
        ELSE 'other'::core.party_relationship_enum
      END
      AND pr.start_date = COALESCE(er.valid_from, CURRENT_DATE)
  );

COMMIT;

-- -----------------------------------------------------------------------------
-- Post-backfill: refresh stats
-- -----------------------------------------------------------------------------
ANALYZE core.parties;
ANALYZE core.people;
ANALYZE core.companies;
ANALYZE core.party_contacts;
ANALYZE core.party_enrichments;
ANALYZE core.ownerships;
ANALYZE core.party_relationships;

-- -----------------------------------------------------------------------------
-- Quick validation
-- -----------------------------------------------------------------------------
SELECT 
  (SELECT COUNT(*) FROM core.parties) AS parties,
  (SELECT COUNT(*) FROM core.people) AS pf,
  (SELECT COUNT(*) FROM core.companies) AS pj,
  (SELECT COUNT(*) FROM core.party_contacts WHERE contact_type = 'email') AS emails,
  (SELECT COUNT(*) FROM core.party_contacts WHERE contact_type IN ('phone','whatsapp')) AS phones,
  (SELECT COUNT(*) FROM core.ownerships) AS ownerships,
  (SELECT COUNT(*) FROM core.party_relationships) AS relationships;

-- -----------------------------------------------------------------------------
-- Duplicate checks (should return zero rows)
-- -----------------------------------------------------------------------------
SELECT 'PF duplicate CPF' AS issue, document_cpf, COUNT(*) 
FROM core.people 
WHERE document_cpf IS NOT NULL
GROUP BY document_cpf HAVING COUNT(*) > 1

UNION ALL

SELECT 'PJ duplicate CNPJ' AS issue, cnpj, COUNT(*) 
FROM core.companies 
WHERE cnpj IS NOT NULL
GROUP BY cnpj HAVING COUNT(*) > 1;
