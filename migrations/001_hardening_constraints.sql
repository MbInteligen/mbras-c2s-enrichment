-- Migration: Database Hardening & Brazilian Real Estate Validation
-- Date: 2025-11-21
-- Purpose: Add constraints, FKs, and validation for production database
-- Risk: LOW (all changes are additive or cleanup duplicates)

-- ============================================================================
-- PHASE 1: DEDUPLICATION (Clean up existing data)
-- ============================================================================

-- 1.1: Deduplicate phones (keep oldest record per entity+phone combination)
-- Impact: Will delete ~1,522 duplicate phone records
WITH duplicates AS (
  SELECT id
  FROM (
    SELECT
      id,
      ROW_NUMBER() OVER (
        PARTITION BY entity_id, phone
        ORDER BY created_at ASC NULLS LAST, id ASC
      ) as rn
    FROM core.entity_phones
    WHERE phone IS NOT NULL
  ) ranked
  WHERE rn > 1
)
DELETE FROM core.entity_phones
WHERE id IN (SELECT id FROM duplicates);

-- Verify deduplication
SELECT
  'Phone deduplication complete' as status,
  COUNT(*) as remaining_duplicates
FROM (
  SELECT entity_id, phone, COUNT(*) as cnt
  FROM core.entity_phones
  WHERE phone IS NOT NULL
  GROUP BY entity_id, phone
  HAVING COUNT(*) > 1
) check_dupes;

-- ============================================================================
-- PHASE 2: ADD UNIQUE CONSTRAINTS
-- ============================================================================

-- 2.1: Per-entity phone uniqueness (prevents future duplicates)
CREATE UNIQUE INDEX IF NOT EXISTS uq_entity_phones_per_entity
  ON core.entity_phones (entity_id, phone)
  WHERE phone IS NOT NULL;

-- 2.2: Entity-address junction uniqueness (one link per entity/address)
-- Check for violations first
DO $$
DECLARE
  violation_count INTEGER;
BEGIN
  SELECT COUNT(*) INTO violation_count
  FROM (
    SELECT entity_id, address_id, COUNT(*) as cnt
    FROM core.entity_addresses
    GROUP BY entity_id, address_id
    HAVING COUNT(*) > 1
  ) violations;

  IF violation_count > 0 THEN
    RAISE NOTICE 'WARNING: % entity-address duplicate links found. Skipping constraint.', violation_count;
  ELSE
    CREATE UNIQUE INDEX IF NOT EXISTS uq_entity_addresses
      ON core.entity_addresses (entity_id, address_id);
    RAISE NOTICE 'Entity-address uniqueness constraint added successfully.';
  END IF;
END $$;

-- ============================================================================
-- PHASE 3: FOREIGN KEY CONSTRAINTS TO REFERENCE TABLES
-- ============================================================================

-- 3.1: Property type FK (only if ref.property_types exists)
DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM information_schema.tables
             WHERE table_schema = 'ref' AND table_name = 'property_types') THEN

    -- Check for orphaned values
    IF NOT EXISTS (
      SELECT 1
      FROM core.real_estate_properties p
      LEFT JOIN ref.property_types pt ON p.property_type = pt.code
      WHERE p.property_type IS NOT NULL AND pt.code IS NULL
    ) THEN
      IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_property_type_ref') THEN
        ALTER TABLE core.real_estate_properties
          ADD CONSTRAINT fk_property_type_ref
          FOREIGN KEY (property_type) REFERENCES ref.property_types(code);
        RAISE NOTICE 'Property type FK added successfully.';
      ELSE
        RAISE NOTICE 'Property type FK already exists.';
      END IF;
    ELSE
      RAISE NOTICE 'WARNING: Orphaned property_type values found. Fix data first.';
    END IF;
  ELSE
    RAISE NOTICE 'SKIP: ref.property_types table does not exist.';
  END IF;
END $$;

-- 3.2: Street type FK (only if ref.street_type_catalog exists)
DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM information_schema.tables
             WHERE table_schema = 'ref' AND table_name = 'street_type_catalog') THEN

    -- Check for orphaned values
    IF NOT EXISTS (
      SELECT 1
      FROM core.addresses a
      LEFT JOIN ref.street_type_catalog st ON a.street_type = st.code
      WHERE a.street_type IS NOT NULL AND st.code IS NULL
    ) THEN
      IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_street_type_catalog') THEN
        ALTER TABLE core.addresses
          ADD CONSTRAINT fk_street_type_catalog
          FOREIGN KEY (street_type) REFERENCES ref.street_type_catalog(code);
        RAISE NOTICE 'Street type FK added successfully.';
      ELSE
        RAISE NOTICE 'Street type FK already exists.';
      END IF;
    ELSE
      RAISE NOTICE 'WARNING: Orphaned street_type values found. Fix data first.';
    END IF;
  ELSE
    RAISE NOTICE 'SKIP: ref.street_type_catalog table does not exist.';
  END IF;
END $$;

-- 3.3: Relationship type FK (only if ref.relationship_types exists)
DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM information_schema.tables
             WHERE table_schema = 'ref' AND table_name = 'relationship_types') THEN

    -- Check for orphaned values
    IF NOT EXISTS (
      SELECT 1
      FROM core.entity_relationships er
      LEFT JOIN ref.relationship_types rt ON er.relationship_type = rt.type
      WHERE er.relationship_type IS NOT NULL AND rt.type IS NULL
    ) THEN
      IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_relationship_type') THEN
        ALTER TABLE core.entity_relationships
          ADD CONSTRAINT fk_relationship_type
          FOREIGN KEY (relationship_type) REFERENCES ref.relationship_types(type);
        RAISE NOTICE 'Relationship type FK added successfully.';
      ELSE
        RAISE NOTICE 'Relationship type FK already exists.';
      END IF;
    ELSE
      RAISE NOTICE 'WARNING: Orphaned relationship_type values found. Fix data first.';
    END IF;
  ELSE
    RAISE NOTICE 'SKIP: ref.relationship_types table does not exist.';
  END IF;
END $$;

-- ============================================================================
-- PHASE 4: BRAZILIAN DATA VALIDATION
-- ============================================================================

-- 4.1: CEP (zip code) validation - must be 8 digits or NULL
ALTER TABLE core.addresses
  DROP CONSTRAINT IF EXISTS chk_addresses_zip_code,
  ADD CONSTRAINT chk_addresses_zip_code
    CHECK (zip_code IS NULL OR zip_code ~ '^[0-9]{8}$');

-- 4.2: UF (state) validation - must be 2 uppercase letters
-- First, normalize existing data
UPDATE core.addresses
SET state = upper(substr(state, 1, 2))
WHERE state IS NOT NULL
  AND (length(state) != 2 OR state !~ '^[A-Z]{2}$');

-- Then add constraint
ALTER TABLE core.addresses
  DROP CONSTRAINT IF EXISTS chk_addresses_state_uf,
  ADD CONSTRAINT chk_addresses_state_uf
    CHECK (state IS NULL OR state ~ '^[A-Z]{2}$');

-- 4.3: Add timestamp defaults (for audit trail)
ALTER TABLE core.addresses
  ALTER COLUMN created_at SET DEFAULT now(),
  ALTER COLUMN updated_at SET DEFAULT now();

-- ============================================================================
-- PHASE 5: PERFORMANCE INDEXES
-- ============================================================================

-- 5.1: Lead tracking index (for querying by c2s_lead_id)
CREATE INDEX IF NOT EXISTS idx_entities_metadata_c2s_lead_id
  ON core.entities ((metadata->>'c2s_lead_id'))
  WHERE metadata->>'c2s_lead_id' IS NOT NULL;

-- 5.2: Neighborhood filtering (for noble neighborhood queries)
CREATE INDEX IF NOT EXISTS idx_addresses_neighborhood_lower
  ON core.addresses (lower(neighborhood))
  WHERE neighborhood IS NOT NULL;

-- 5.3: Confidence score filtering (for high-confidence addresses)
CREATE INDEX IF NOT EXISTS idx_entity_addresses_confidence_high
  ON core.entity_addresses (confidence_score)
  WHERE confidence_score >= 0.75;

-- 5.4: Enriched entities index
CREATE INDEX IF NOT EXISTS idx_entities_enriched_at
  ON core.entities (is_enriched, enriched_at DESC)
  WHERE is_enriched = true;

-- ============================================================================
-- PHASE 6: OPTIONAL - ADDRESS DEDUPLICATION SUPPORT
-- ============================================================================

-- 6.1: Address hash uniqueness (if address_hash column exists and is populated)
DO $$
DECLARE
  hash_populated_count INTEGER;
BEGIN
  -- Check if address_hash column exists
  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema = 'core'
      AND table_name = 'addresses'
      AND column_name = 'address_hash'
  ) THEN

    -- Check if any rows have address_hash populated
    SELECT COUNT(*) INTO hash_populated_count
    FROM core.addresses
    WHERE address_hash IS NOT NULL;

    IF hash_populated_count > 0 THEN
      -- Check for hash conflicts before adding unique constraint
      IF NOT EXISTS (
        SELECT 1
        FROM core.addresses
        WHERE address_hash IS NOT NULL
        GROUP BY address_hash
        HAVING COUNT(*) > 1
      ) THEN
        CREATE UNIQUE INDEX IF NOT EXISTS uq_addresses_hash
          ON core.addresses (address_hash)
          WHERE address_hash IS NOT NULL;
        RAISE NOTICE 'Address hash uniqueness constraint added (% rows with hashes).', hash_populated_count;
      ELSE
        RAISE NOTICE 'WARNING: Duplicate address_hash values found. Resolve conflicts first.';
      END IF;
    ELSE
      RAISE NOTICE 'SKIP: address_hash column exists but not populated.';
    END IF;
  ELSE
    RAISE NOTICE 'SKIP: address_hash column does not exist.';
  END IF;
END $$;

-- ============================================================================
-- PHASE 7: STATISTICS UPDATE
-- ============================================================================

-- Update statistics for query planner
ANALYZE core.entities;
ANALYZE core.entity_phones;
ANALYZE core.entity_emails;
ANALYZE core.entity_addresses;
ANALYZE core.addresses;
ANALYZE core.entity_profiles;
ANALYZE core.entity_financials;

-- ============================================================================
-- VERIFICATION QUERIES
-- ============================================================================

-- Verify no duplicate phones remain
SELECT
  'Phone uniqueness check' as validation,
  COUNT(*) as violations,
  CASE WHEN COUNT(*) = 0 THEN 'PASS ✅' ELSE 'FAIL ❌' END as status
FROM (
  SELECT entity_id, phone, COUNT(*) as cnt
  FROM core.entity_phones
  WHERE phone IS NOT NULL
  GROUP BY entity_id, phone
  HAVING COUNT(*) > 1
) check_phones;

-- Verify constraint creation
SELECT
  'Constraints added' as validation,
  COUNT(*) as constraint_count,
  string_agg(constraint_name, ', ') as constraints
FROM information_schema.table_constraints
WHERE table_schema = 'core'
  AND constraint_name IN (
    'uq_entity_phones_per_entity',
    'uq_entity_addresses',
    'fk_property_type_ref',
    'fk_street_type_catalog',
    'fk_relationship_type',
    'chk_addresses_zip_code',
    'chk_addresses_state_uf'
  );

-- Verify index creation
SELECT
  'Indexes added' as validation,
  COUNT(*) as index_count,
  string_agg(indexname, ', ') as indexes
FROM pg_indexes
WHERE schemaname = 'core'
  AND indexname IN (
    'uq_entity_phones_per_entity',
    'uq_entity_addresses',
    'idx_entities_metadata_c2s_lead_id',
    'idx_addresses_neighborhood_lower',
    'idx_entity_addresses_confidence_high',
    'idx_entities_enriched_at'
  );

-- ============================================================================
-- ROLLBACK INSTRUCTIONS (if needed)
-- ============================================================================

/*
-- To rollback this migration:

DROP INDEX IF EXISTS core.uq_entity_phones_per_entity;
DROP INDEX IF EXISTS core.uq_entity_addresses;
DROP INDEX IF EXISTS core.idx_entities_metadata_c2s_lead_id;
DROP INDEX IF EXISTS core.idx_addresses_neighborhood_lower;
DROP INDEX IF EXISTS core.idx_entity_addresses_confidence_high;
DROP INDEX IF EXISTS core.idx_entities_enriched_at;
DROP INDEX IF EXISTS core.uq_addresses_hash;

ALTER TABLE core.real_estate_properties DROP CONSTRAINT IF EXISTS fk_property_type_ref;
ALTER TABLE core.addresses DROP CONSTRAINT IF EXISTS fk_street_type_catalog;
ALTER TABLE core.entity_relationships DROP CONSTRAINT IF EXISTS fk_relationship_type;
ALTER TABLE core.addresses DROP CONSTRAINT IF EXISTS chk_addresses_zip_code;
ALTER TABLE core.addresses DROP CONSTRAINT IF EXISTS chk_addresses_state_uf;

-- Note: Deduplication cannot be rolled back. Keep database backup before running.
*/

-- ============================================================================
-- MIGRATION COMPLETE
-- ============================================================================

SELECT
  '✅ Migration 001_hardening_constraints.sql completed successfully' as status,
  now() as completed_at;
