-- Migration 018: Drop unused app and dim schemas
-- Date: 2025-11-22
-- Note: Ensure no external dependencies on app.emails/phones/iptus or dim schema before applying.

BEGIN;

DROP SCHEMA IF EXISTS dim CASCADE;
DROP SCHEMA IF EXISTS app CASCADE;

COMMIT;
