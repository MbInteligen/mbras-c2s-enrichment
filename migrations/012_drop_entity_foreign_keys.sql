-- Migration 012: Drop legacy FK constraints pointing to core.entities
-- Date: 2025-11-22
-- Purpose: Prepare for deprecating legacy entity tables by removing FKs

BEGIN;

DO $$
DECLARE
    cmd text;
BEGIN
    FOR cmd IN
        SELECT
            format('ALTER TABLE %I.%I DROP CONSTRAINT %I;',
                   n.nspname, c.relname, con.conname)
        FROM pg_constraint con
        JOIN pg_class c ON c.oid = con.conrelid
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE con.confrelid = 'core.entities'::regclass
    LOOP
        EXECUTE cmd;
    END LOOP;
END $$;

-- Verification: list remaining FKs to core.entities (should be zero)
DO $$
DECLARE
    remaining integer;
BEGIN
    SELECT COUNT(*) INTO remaining
    FROM pg_constraint
    WHERE confrelid = 'core.entities'::regclass;

    RAISE NOTICE 'Remaining FKs to core.entities: %', remaining;
END $$;

COMMIT;
