-- Migration 013: Drop unused split contact tables (party_emails, party_phones, party_iptus)
-- Date: 2025-11-22
-- Note: Unified contacts are stored in core.party_contacts

BEGIN;

DROP TABLE IF EXISTS core.party_emails CASCADE;
DROP TABLE IF EXISTS core.party_phones CASCADE;
DROP TABLE IF EXISTS core.party_iptus CASCADE;

COMMIT;
