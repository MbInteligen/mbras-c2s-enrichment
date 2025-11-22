-- Migration 011: Archive Legacy Entity Tables
-- Created: 2025-11-22
-- Purpose: Move entity_* tables to archive schema after Party Model migration complete
--
-- IMPORTANT: This migration should only be run AFTER:
-- 1. Phase 1-2 code deployed and tested in production
-- 2. 90-day monitoring period confirms no entity_* table usage
-- 3. All code audited to use party_* tables instead
--
-- This migration:
-- 1. Creates archive schema
-- 2. Drops FK constraint from entity_addresses
-- 3. Moves all entity_* tables to archive schema
-- 4. Documents archived tables for future reference

-- ============================================================
-- STEP 1: Create Archive Schema
-- ============================================================

CREATE SCHEMA IF NOT EXISTS archive;

COMMENT ON SCHEMA archive IS
'Archive schema for deprecated tables from Party Model migration.
Tables moved here on 2025-11-22 after 90-day monitoring period.
Tables in this schema are read-only historical records.
Safe to drop after additional 90-day archive retention period (2026-02-22).';

-- ============================================================
-- STEP 2: Drop Foreign Key Constraint
-- ============================================================

-- Drop FK from entity_addresses to core.addresses
-- This is the only FK constraint from entity_* tables
ALTER TABLE core.entity_addresses
DROP CONSTRAINT IF EXISTS entity_addresses_address_id_fkey;

-- ============================================================
-- STEP 3: Move Entity Tables to Archive Schema
-- ============================================================

-- Move entity_addresses (319 MB, 11,530 rows)
ALTER TABLE core.entity_addresses SET SCHEMA archive;
COMMENT ON TABLE archive.entity_addresses IS
'ARCHIVED 2025-11-22: Legacy address relationships from entity model.
Replaced by core.party_addresses. All 11,530 rows migrated in migration 009.
Original size: 319 MB.';

-- Move entity_emails (371 MB, 2,595,091 rows)
ALTER TABLE core.entity_emails SET SCHEMA archive;
COMMENT ON TABLE archive.entity_emails IS
'ARCHIVED 2025-11-22: Legacy email relationships from entity model.
Replaced by core.party_emails. All rows migrated.
Original size: 371 MB.';

-- Move entity_family_relationships (259 MB)
ALTER TABLE core.entity_family_relationships SET SCHEMA archive;
COMMENT ON TABLE archive.entity_family_relationships IS
'ARCHIVED 2025-11-22: Legacy family relationships from entity model.
Replaced by core.party_relationships with relationship_type.
Original size: 259 MB.';

-- Move entity_financials (360 KB, 250 rows)
ALTER TABLE core.entity_financials SET SCHEMA archive;
COMMENT ON TABLE archive.entity_financials IS
'ARCHIVED 2025-11-22: Legacy financial data from entity model.
Replaced by JSONB in core.party_enrichments.normalized_data[''financials''].
All 250 rows migrated in migration 010.
Original size: 360 KB.';

-- Move entity_phones (724 MB, 2,595,235 rows)
ALTER TABLE core.entity_phones SET SCHEMA archive;
COMMENT ON TABLE archive.entity_phones IS
'ARCHIVED 2025-11-22: Legacy phone relationships from entity model.
Replaced by core.party_phones. All rows migrated.
Original size: 724 MB.';

-- Move entity_profiles (517 MB, 695,310 rows)
ALTER TABLE core.entity_profiles SET SCHEMA archive;
COMMENT ON TABLE archive.entity_profiles IS
'ARCHIVED 2025-11-22: Legacy enrichment profiles from entity model.
Replaced by core.party_enrichments. All rows migrated.
Original size: 517 MB.';

-- Move entity_relationships (192 MB)
ALTER TABLE core.entity_relationships SET SCHEMA archive;
COMMENT ON TABLE archive.entity_relationships IS
'ARCHIVED 2025-11-22: Legacy general relationships from entity model.
Replaced by core.party_relationships.
Original size: 192 MB.';

-- ============================================================
-- STEP 4: Create Archive Metadata Table
-- ============================================================

CREATE TABLE IF NOT EXISTS archive.archive_metadata (
    table_name TEXT PRIMARY KEY,
    archived_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    original_schema TEXT NOT NULL,
    original_row_count BIGINT,
    original_size_bytes BIGINT,
    replacement_table TEXT,
    migration_id INTEGER,
    safe_to_drop_after DATE,
    notes TEXT
);

COMMENT ON TABLE archive.archive_metadata IS
'Metadata tracking for archived tables. Documents when tables were archived,
their original stats, and when they can be safely dropped.';

-- ============================================================
-- STEP 5: Document Archived Tables
-- ============================================================

INSERT INTO archive.archive_metadata (
    table_name,
    original_schema,
    original_row_count,
    original_size_bytes,
    replacement_table,
    migration_id,
    safe_to_drop_after,
    notes
) VALUES
(
    'entity_addresses',
    'core',
    11530,
    334102528,  -- 319 MB
    'core.party_addresses',
    9,
    CURRENT_DATE + INTERVAL '90 days',
    'Archived after 90-day monitoring period. All rows migrated to party_addresses in migration 009.'
),
(
    'entity_emails',
    'core',
    2595091,
    388923392,  -- 371 MB
    'core.party_emails',
    NULL,
    CURRENT_DATE + INTERVAL '90 days',
    'Archived after Party Model migration. Email relationships now managed through party_emails.'
),
(
    'entity_family_relationships',
    'core',
    NULL,
    271581184,  -- 259 MB
    'core.party_relationships',
    NULL,
    CURRENT_DATE + INTERVAL '90 days',
    'Archived after Party Model migration. Family relationships merged into party_relationships with relationship_type.'
),
(
    'entity_financials',
    'core',
    250,
    368640,  -- 360 KB
    'core.party_enrichments (JSONB)',
    10,
    CURRENT_DATE + INTERVAL '90 days',
    'Archived after financial data migrated to JSONB in migration 010. Only 250 records existed.'
),
(
    'entity_phones',
    'core',
    2595235,
    758972416,  -- 724 MB
    'core.party_phones',
    NULL,
    CURRENT_DATE + INTERVAL '90 days',
    'Archived after Party Model migration. Phone relationships now managed through party_phones.'
),
(
    'entity_profiles',
    'core',
    695310,
    542113792,  -- 517 MB
    'core.party_enrichments',
    NULL,
    CURRENT_DATE + INTERVAL '90 days',
    'Archived after Party Model migration. Enrichment data consolidated in party_enrichments.'
),
(
    'entity_relationships',
    'core',
    NULL,
    201326592,  -- 192 MB
    'core.party_relationships',
    NULL,
    CURRENT_DATE + INTERVAL '90 days',
    'Archived after Party Model migration. General relationships now in party_relationships.'
);

-- ============================================================
-- STEP 6: Create Archive Monitoring Views
-- ============================================================

-- View to check what's safe to drop
CREATE OR REPLACE VIEW archive.safe_to_drop AS
SELECT
    table_name,
    archived_at,
    safe_to_drop_after,
    CASE
        WHEN safe_to_drop_after <= CURRENT_DATE THEN 'YES - Safe to drop'
        ELSE 'NO - Wait until ' || safe_to_drop_after::text
    END as drop_status,
    safe_to_drop_after - CURRENT_DATE as days_remaining,
    replacement_table,
    notes
FROM archive.archive_metadata
ORDER BY safe_to_drop_after;

COMMENT ON VIEW archive.safe_to_drop IS
'Shows which archived tables are past their retention period and safe to drop.
Check this view before running cleanup migrations.';

-- View to monitor archive schema size
CREATE OR REPLACE VIEW archive.archive_size_report AS
SELECT
    am.table_name,
    pg_size_pretty(pg_total_relation_size('archive.' || am.table_name)) as current_size,
    pg_size_pretty(am.original_size_bytes) as original_size,
    am.original_row_count,
    (SELECT COUNT(*) FROM information_schema.columns
     WHERE table_schema = 'archive' AND table_name = am.table_name) as column_count,
    am.archived_at,
    am.replacement_table
FROM archive.archive_metadata am
ORDER BY pg_total_relation_size('archive.' || am.table_name) DESC;

COMMENT ON VIEW archive.archive_size_report IS
'Shows current size of archived tables for monitoring disk usage.
Total archive schema size shown at bottom.';

-- ============================================================
-- STEP 7: Grant Permissions
-- ============================================================

-- Archive schema is read-only
GRANT USAGE ON SCHEMA archive TO PUBLIC;
GRANT SELECT ON ALL TABLES IN SCHEMA archive TO PUBLIC;

-- Prevent modifications
REVOKE INSERT, UPDATE, DELETE, TRUNCATE ON ALL TABLES IN SCHEMA archive FROM PUBLIC;

-- ============================================================
-- FINAL VERIFICATION
-- ============================================================

-- Log migration completion
DO $$
DECLARE
    total_size BIGINT;
    table_count INTEGER;
BEGIN
    SELECT
        COUNT(*),
        SUM(pg_total_relation_size('archive.' || table_name))
    INTO table_count, total_size
    FROM archive.archive_metadata;

    RAISE NOTICE '';
    RAISE NOTICE '========================================';
    RAISE NOTICE 'Migration 011: Archive Entity Tables';
    RAISE NOTICE '========================================';
    RAISE NOTICE 'Tables archived: %', table_count;
    RAISE NOTICE 'Total archive size: %', pg_size_pretty(total_size);
    RAISE NOTICE 'Archive schema: created';
    RAISE NOTICE 'Safe to drop after: %', (CURRENT_DATE + INTERVAL '90 days')::text;
    RAISE NOTICE '';
    RAISE NOTICE 'Archived tables:';
    RAISE NOTICE '  - entity_addresses (319 MB)';
    RAISE NOTICE '  - entity_emails (371 MB)';
    RAISE NOTICE '  - entity_family_relationships (259 MB)';
    RAISE NOTICE '  - entity_financials (360 KB)';
    RAISE NOTICE '  - entity_phones (724 MB)';
    RAISE NOTICE '  - entity_profiles (517 MB)';
    RAISE NOTICE '  - entity_relationships (192 MB)';
    RAISE NOTICE '';
    RAISE NOTICE 'Next steps:';
    RAISE NOTICE '  1. Monitor production for any errors';
    RAISE NOTICE '  2. Wait 90 days (until %) for safety', (CURRENT_DATE + INTERVAL '90 days')::text;
    RAISE NOTICE '  3. Query archive.safe_to_drop view';
    RAISE NOTICE '  4. Create migration 012 to DROP archived tables';
    RAISE NOTICE '========================================';
END $$;
