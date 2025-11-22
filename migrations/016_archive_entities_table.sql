-- Migration 016: Move core.entities to archive schema
-- Date: 2025-11-22
-- Purpose: Archive legacy entities table after party model cutover
-- Note: Ensure no code depends on core.entities before applying.

BEGIN;

-- Create archive schema if missing
CREATE SCHEMA IF NOT EXISTS archive;

-- Move core.entities to archive schema
ALTER TABLE IF EXISTS core.entities SET SCHEMA archive;

-- Document in archive.archive_metadata if present
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_schema='archive' AND table_name='archive_metadata'
    ) THEN
        INSERT INTO archive.archive_metadata (
            table_name,
            original_schema,
            original_row_count,
            original_size_bytes,
            replacement_table,
            migration_id,
            safe_to_drop_after,
            notes
        )
        SELECT
            'entities',
            'core',
            (SELECT COUNT(*) FROM archive.entities),
            NULL,
            'core.parties',
            16,
            CURRENT_DATE + INTERVAL '90 days',
            'Archived legacy entities table after party model cutover'
        ON CONFLICT (table_name) DO NOTHING;
    END IF;
END $$;

COMMIT;
