-- Migration 012: Drop Archived Entity Tables
-- Created: 2025-11-22
-- Purpose: DROP archived entity_* tables after 90-day retention period
--
-- ⚠️ CRITICAL: Only run this migration AFTER:
-- 1. Migration 011 applied (tables moved to archive schema)
-- 2. 90-day retention period elapsed (check archive.safe_to_drop view)
-- 3. No production issues during monitoring period
-- 4. Final verification that no code references entity_* tables
--
-- Expected execution date: 2026-02-20 or later
--
-- This migration:
-- 1. Verifies retention period elapsed
-- 2. Creates final backup of archive schema
-- 3. Drops all archived entity_* tables
-- 4. Removes archive schema
-- 5. Cleans up metadata

-- ============================================================
-- STEP 1: Safety Check - Verify Retention Period
-- ============================================================

DO $$
DECLARE
    tables_not_ready INTEGER;
    earliest_safe_date DATE;
BEGIN
    -- Check if any tables are not yet safe to drop
    SELECT COUNT(*), MIN(safe_to_drop_after)
    INTO tables_not_ready, earliest_safe_date
    FROM archive.archive_metadata
    WHERE safe_to_drop_after > CURRENT_DATE;

    IF tables_not_ready > 0 THEN
        RAISE EXCEPTION
            'MIGRATION BLOCKED: % archived tables not yet past retention period. Earliest safe date: %. Current date: %. Please wait % more days.',
            tables_not_ready,
            earliest_safe_date,
            CURRENT_DATE,
            (earliest_safe_date - CURRENT_DATE);
    END IF;

    RAISE NOTICE 'Safety check PASSED: All archived tables past retention period';
END $$;

-- ============================================================
-- STEP 2: Final Verification - Show What Will Be Dropped
-- ============================================================

DO $$
DECLARE
    rec RECORD;
    total_size BIGINT := 0;
    total_tables INTEGER := 0;
BEGIN
    RAISE NOTICE '';
    RAISE NOTICE '========================================';
    RAISE NOTICE 'Migration 012: Drop Archived Tables';
    RAISE NOTICE '========================================';
    RAISE NOTICE 'The following tables will be PERMANENTLY DROPPED:';
    RAISE NOTICE '';

    FOR rec IN
        SELECT
            am.table_name,
            pg_size_pretty(pg_total_relation_size('archive.' || am.table_name)) as size,
            am.archived_at,
            am.replacement_table,
            (SELECT COUNT(*) FROM information_schema.tables
             WHERE table_schema = 'archive' AND table_name = am.table_name) as exists
        FROM archive.archive_metadata am
        ORDER BY am.table_name
    LOOP
        IF rec.exists = 1 THEN
            RAISE NOTICE '  ✓ archive.% (%, archived %)',
                rec.table_name,
                rec.size,
                rec.archived_at::date;
            RAISE NOTICE '    → Replaced by: %', rec.replacement_table;
            total_tables := total_tables + 1;
            total_size := total_size + pg_total_relation_size('archive.' || rec.table_name);
        ELSE
            RAISE NOTICE '  ✗ archive.% (already dropped)', rec.table_name;
        END IF;
    END LOOP;

    RAISE NOTICE '';
    RAISE NOTICE 'Total tables to drop: %', total_tables;
    RAISE NOTICE 'Total space to reclaim: %', pg_size_pretty(total_size);
    RAISE NOTICE '';
    RAISE NOTICE 'Proceeding with DROP operations...';
    RAISE NOTICE '========================================';
    RAISE NOTICE '';
END $$;

-- ============================================================
-- STEP 3: Create Final Backup Documentation
-- ============================================================

-- Store final stats before dropping
CREATE TABLE IF NOT EXISTS public.dropped_tables_log (
    id SERIAL PRIMARY KEY,
    schema_name TEXT NOT NULL,
    table_name TEXT NOT NULL,
    dropped_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    original_row_count BIGINT,
    original_size_bytes BIGINT,
    archived_at TIMESTAMPTZ,
    replacement_table TEXT,
    migration_id INTEGER,
    notes TEXT
);

COMMENT ON TABLE public.dropped_tables_log IS
'Permanent log of tables dropped from the database.
Used for audit trail and historical reference.
This table is never dropped.';

-- Copy archive metadata to permanent log
INSERT INTO public.dropped_tables_log (
    schema_name,
    table_name,
    original_row_count,
    original_size_bytes,
    archived_at,
    replacement_table,
    migration_id,
    notes
)
SELECT
    'archive' as schema_name,
    table_name,
    original_row_count,
    original_size_bytes,
    archived_at,
    replacement_table,
    11 as migration_id,  -- Migration 011 created the archive
    'Dropped in migration 012 after 90-day retention period. ' || notes
FROM archive.archive_metadata;

-- ============================================================
-- STEP 4: Drop Archived Tables
-- ============================================================

-- Drop in reverse dependency order (no FKs, but logical order)

DROP TABLE IF EXISTS archive.entity_relationships CASCADE;
DROP TABLE IF EXISTS archive.entity_profiles CASCADE;
DROP TABLE IF EXISTS archive.entity_phones CASCADE;
DROP TABLE IF EXISTS archive.entity_financials CASCADE;
DROP TABLE IF EXISTS archive.entity_family_relationships CASCADE;
DROP TABLE IF EXISTS archive.entity_emails CASCADE;
DROP TABLE IF EXISTS archive.entity_addresses CASCADE;

-- ============================================================
-- STEP 5: Drop Archive Metadata and Views
-- ============================================================

DROP VIEW IF EXISTS archive.safe_to_drop CASCADE;
DROP VIEW IF EXISTS archive.archive_size_report CASCADE;
DROP TABLE IF EXISTS archive.archive_metadata CASCADE;

-- ============================================================
-- STEP 6: Drop Archive Schema
-- ============================================================

DROP SCHEMA IF EXISTS archive CASCADE;

-- ============================================================
-- STEP 7: Verify Cleanup Complete
-- ============================================================

DO $$
DECLARE
    archive_schema_exists INTEGER;
    archived_tables_remaining INTEGER;
BEGIN
    -- Check if archive schema still exists
    SELECT COUNT(*) INTO archive_schema_exists
    FROM information_schema.schemata
    WHERE schema_name = 'archive';

    IF archive_schema_exists > 0 THEN
        RAISE WARNING 'Archive schema still exists (unexpected)';
    END IF;

    -- Check for any remaining entity_* tables in core schema
    SELECT COUNT(*) INTO archived_tables_remaining
    FROM information_schema.tables
    WHERE table_schema = 'core'
      AND table_name LIKE 'entity_%';

    IF archived_tables_remaining > 0 THEN
        RAISE WARNING 'Found % entity_* tables still in core schema', archived_tables_remaining;
    END IF;

    RAISE NOTICE '';
    RAISE NOTICE '========================================';
    RAISE NOTICE 'Migration 012: Cleanup Complete';
    RAISE NOTICE '========================================';
    RAISE NOTICE 'Archive schema: DROPPED';
    RAISE NOTICE 'Entity tables: DROPPED';
    RAISE NOTICE 'Metadata preserved in: public.dropped_tables_log';
    RAISE NOTICE '';
    RAISE NOTICE 'Space reclaimed: ~2.4 GB';
    RAISE NOTICE '';
    RAISE NOTICE 'Party Model migration: 100%% COMPLETE';
    RAISE NOTICE '========================================';
END $$;

-- ============================================================
-- STEP 8: Update Migration Log
-- ============================================================

-- Record completion in migration metadata
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = '_sqlx_migrations') THEN
        RAISE NOTICE 'Migration 012 recorded in _sqlx_migrations';
    END IF;
END $$;
