-- Migration: Google Ads Lead Integration Tracking
-- Purpose: Track Google Ads leads, enrichment status, and C2S integration
-- Date: 2025-01-21

-- Create google_ads_leads table
CREATE TABLE IF NOT EXISTS google_ads_leads (
    -- Primary identifier
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Google Ads identifiers
    google_lead_id TEXT NOT NULL,           -- Unique lead ID from Google Ads (deduplication key)
    form_id BIGINT NOT NULL,                -- Google Ads form ID
    campaign_id BIGINT NOT NULL,            -- Google Ads campaign ID
    gcl_id TEXT,                            -- Google Click ID (gclid) for conversion tracking

    -- C2S integration
    c2s_lead_id TEXT,                       -- Lead ID in Contact2Sale CRM
    c2s_created_at TIMESTAMPTZ,             -- When lead was created in C2S
    c2s_latency_ms INT,                     -- C2S API call latency in milliseconds

    -- Enrichment tracking
    enrichment_status TEXT NOT NULL DEFAULT 'pending',  -- pending, completed, partial, failed
    cpf TEXT,                               -- CPF found during enrichment (if any)

    -- Data storage
    payload_raw JSONB NOT NULL,             -- Complete webhook payload from Google Ads
    description_length INT,                 -- Length of description sent to C2S

    -- Error tracking
    error_message TEXT,                     -- Error message if processing failed

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- Unique constraint for idempotency (prevent duplicate processing)
    CONSTRAINT uq_google_lead_id UNIQUE (google_lead_id)
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_google_ads_leads_campaign_id
    ON google_ads_leads (campaign_id);

CREATE INDEX IF NOT EXISTS idx_google_ads_leads_form_id
    ON google_ads_leads (form_id);

CREATE INDEX IF NOT EXISTS idx_google_ads_leads_c2s_lead_id
    ON google_ads_leads (c2s_lead_id);

CREATE INDEX IF NOT EXISTS idx_google_ads_leads_enrichment_status
    ON google_ads_leads (enrichment_status);

CREATE INDEX IF NOT EXISTS idx_google_ads_leads_created_at
    ON google_ads_leads (created_at DESC);

CREATE INDEX IF NOT EXISTS idx_google_ads_leads_gcl_id
    ON google_ads_leads (gcl_id)
    WHERE gcl_id IS NOT NULL;

-- Comments for documentation
COMMENT ON TABLE google_ads_leads IS 'Tracks Google Ads leads received via webhook, enrichment status, and C2S integration';
COMMENT ON COLUMN google_ads_leads.google_lead_id IS 'Unique identifier from Google Ads (used for deduplication)';
COMMENT ON COLUMN google_ads_leads.enrichment_status IS 'Status: pending, completed (full enrichment), partial (limited enrichment), failed';
COMMENT ON COLUMN google_ads_leads.c2s_latency_ms IS 'Time taken to create lead in C2S (milliseconds)';
COMMENT ON COLUMN google_ads_leads.description_length IS 'Character count of description sent to C2S (for tracking truncation)';
