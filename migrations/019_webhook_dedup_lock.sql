-- Migration 019: Add unique constraint + advisory lock support for webhook dedup
-- Date: 2026-02-14
-- Purpose: Prevents duplicate webhook processing across multiple Fly.io instances.
-- The UNIQUE constraint enables INSERT ... ON CONFLICT DO NOTHING (atomic dedup).

BEGIN;

-- 1. Add unique constraint on (lead_id, updated_at) for atomic dedup
-- If duplicate rows already exist, deduplicate first by keeping the earliest.
DELETE FROM webhook_events a
USING webhook_events b
WHERE a.lead_id = b.lead_id
  AND a.updated_at = b.updated_at
  AND a.ctid > b.ctid;

CREATE UNIQUE INDEX IF NOT EXISTS uq_webhook_events_lead_updated
    ON webhook_events (lead_id, updated_at);

COMMIT;
