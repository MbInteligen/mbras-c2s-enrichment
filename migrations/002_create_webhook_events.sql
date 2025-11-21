-- Migration: Create webhook_events table for C2S webhook persistence
-- Purpose: Store and deduplicate incoming webhook events from Contact2Sale
-- Date: 2025-01-20

-- Create webhook_events table
CREATE TABLE IF NOT EXISTS webhook_events (
    -- Primary key
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Event identifiers (for idempotency)
    lead_id TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,

    -- Event metadata
    hook_action TEXT,  -- e.g., "lead.created", "lead.updated"

    -- Raw payload (JSONB for efficient querying)
    payload_raw JSONB NOT NULL,

    -- Processing metadata
    received_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    processed_at TIMESTAMPTZ,
    status TEXT NOT NULL DEFAULT 'received',  -- received, processing, completed, failed
    error_message TEXT,

    -- Audit fields
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at_ts TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Create unique index for idempotency (lead_id + updated_at)
-- This prevents duplicate webhook processing
CREATE UNIQUE INDEX IF NOT EXISTS ux_webhook_events_lead_updated
    ON webhook_events (lead_id, updated_at);

-- Create index on status for querying unprocessed events
CREATE INDEX IF NOT EXISTS ix_webhook_events_status
    ON webhook_events (status) WHERE status IN ('received', 'processing');

-- Create index on received_at for time-based queries
CREATE INDEX IF NOT EXISTS ix_webhook_events_received_at
    ON webhook_events (received_at DESC);

-- Create index on lead_id for lead-specific queries
CREATE INDEX IF NOT EXISTS ix_webhook_events_lead_id
    ON webhook_events (lead_id);

-- Add comment explaining idempotency strategy
COMMENT ON INDEX ux_webhook_events_lead_updated IS
    'Ensures idempotency: same lead_id + updated_at = duplicate webhook (ignore)';

COMMENT ON TABLE webhook_events IS
    'Stores incoming C2S webhook events with idempotency and processing status tracking';
