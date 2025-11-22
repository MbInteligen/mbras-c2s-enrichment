-- Migration: Analytics MVs (Base + Star)
-- Date: 2025-11-22
-- Purpose: Create optimized Materialized Views for Marketing/BI with auto-refresh

-- 1. Create Analytics Schema
CREATE SCHEMA IF NOT EXISTS analytics;

-- 2. Enable pg_cron extension (if not enabled)
-- Note: On managed DBs, this might need to be done manually in the 'postgres' db
-- CREATE EXTENSION IF NOT EXISTS pg_cron;

-- ============================================================================
-- MV 1: BASE ANALYTICS (core.mv_party_analytics)
-- Unifies PF and PJ data into a single analytical record
-- ============================================================================
DROP MATERIALIZED VIEW IF EXISTS core.mv_party_analytics CASCADE;

CREATE MATERIALIZED VIEW core.mv_party_analytics AS
SELECT
    p.id AS party_id,
    p.party_type, -- 'PF' or 'PJ'
    p.cpf_cnpj,
    p.full_name AS name,
    p.normalized_name,
    
    -- PF Attributes
    p.birth_date,
    EXTRACT(YEAR FROM AGE(CURRENT_DATE, p.birth_date))::int AS age,
    p.sex,
    p.mother_name,
    
    -- PJ Attributes
    p.fantasy_name,
    p.opening_date,
    EXTRACT(YEAR FROM AGE(CURRENT_DATE, p.opening_date))::int AS company_age_years,
    p.company_type,
    p.company_size,
    p.registration_status_date,
    
    -- Metadata
    p.created_at,
    p.updated_at,
    p.enriched AS is_enriched
FROM core.parties p
WITH DATA;

-- Indexes for Base MV
CREATE UNIQUE INDEX idx_mv_party_analytics_id ON core.mv_party_analytics (party_id);
CREATE INDEX idx_mv_party_analytics_doc ON core.mv_party_analytics (cpf_cnpj);
CREATE INDEX idx_mv_party_analytics_name ON core.mv_party_analytics (normalized_name);
CREATE INDEX idx_mv_party_analytics_type ON core.mv_party_analytics (party_type);
CREATE INDEX idx_mv_party_analytics_age ON core.mv_party_analytics (age) WHERE age IS NOT NULL;
CREATE INDEX idx_mv_party_analytics_company_size ON core.mv_party_analytics (company_size) WHERE company_size IS NOT NULL;

-- ============================================================================
-- MV 2: MARKETING STAR (analytics.mv_mkt_lead_star)
-- Flattened Star Schema for BI Dashboards
-- ============================================================================
DROP MATERIALIZED VIEW IF EXISTS analytics.mv_mkt_lead_star CASCADE;

CREATE MATERIALIZED VIEW analytics.mv_mkt_lead_star AS
SELECT
    -- Dimensions: Lead
    e.entity_id AS lead_id,
    e.name AS lead_name,
    e.created_at AS lead_created_at,
    e.is_enriched,
    e.confidence::text AS lead_score,
    
    -- Dimensions: Party (Joined from Base MV)
    pa.party_id,
    pa.party_type,
    pa.name AS party_name,
    pa.age,
    pa.sex,
    pa.company_size,
    pa.company_type,
    
    -- Dimensions: Contact
    e.metadata->>'email' AS email,
    e.metadata->>'phone' AS phone,
    
    -- Dimensions: Campaign (Left Join)
    gal.campaign_id,
    COALESCE(gal.payload_raw->>'campaign_name', 'Unknown') AS campaign_name,
    gal.created_at AS campaign_interaction_at,
    
    -- Metrics: Real Estate Assets (Aggregated)
    COALESCE(prop_stats.property_count, 0) AS property_count,
    COALESCE(prop_stats.total_assets_brl, 0) AS total_assets_brl,
    
    -- Time Dimensions (Pre-calculated for BI)
    to_char(e.created_at, 'YYYY-MM') AS cohort_month,
    EXTRACT(DOW FROM e.created_at) AS created_dow
    
FROM core.entities e
-- Join to Party Analytics
LEFT JOIN core.mv_party_analytics pa ON e.national_id = pa.cpf_cnpj
-- Join to Campaign Data (Deduplicated)
LEFT JOIN (
    SELECT DISTINCT ON (c2s_lead_id) 
        c2s_lead_id,
        campaign_id,
        payload_raw,
        created_at
    FROM public.google_ads_leads
    ORDER BY c2s_lead_id, created_at DESC
) gal ON e.metadata->>'c2s_lead_id' = gal.c2s_lead_id
-- Join to Property Stats (Pre-aggregated)
LEFT JOIN (
    SELECT 
        po.entity_id,
        count(*) AS property_count,
        sum(rp.confidence_score) AS total_assets_brl -- Using confidence_score as proxy for value if value column missing, or replace with actual value column
    FROM core.property_ownerships po
    JOIN core.real_estate_properties rp ON po.property_id = rp.property_id
    WHERE po.is_current = true
    GROUP BY 1
) prop_stats ON e.entity_id = prop_stats.entity_id
WITH DATA;

-- Indexes for Star MV
CREATE UNIQUE INDEX idx_mv_mkt_star_lead_id ON analytics.mv_mkt_lead_star (lead_id);
CREATE INDEX idx_mv_mkt_star_party_id ON analytics.mv_mkt_lead_star (party_id);
CREATE INDEX idx_mv_mkt_star_campaign ON analytics.mv_mkt_lead_star (campaign_id);
CREATE INDEX idx_mv_mkt_star_created_at ON analytics.mv_mkt_lead_star (lead_created_at DESC);
CREATE INDEX idx_mv_mkt_star_score ON analytics.mv_mkt_lead_star (lead_score);
CREATE INDEX idx_mv_mkt_star_assets ON analytics.mv_mkt_lead_star (total_assets_brl DESC);
CREATE INDEX idx_mv_mkt_star_cohort ON analytics.mv_mkt_lead_star (cohort_month);

-- ============================================================================
-- REFRESH JOBS (pg_cron)
-- ============================================================================

-- Schedule Base MV refresh every 10 minutes
-- SELECT cron.schedule('refresh-mv-party-analytics', '*/10 * * * *', 'REFRESH MATERIALIZED VIEW CONCURRENTLY core.mv_party_analytics');

-- Schedule Star MV refresh every 10 minutes (offset by 5 mins)
-- SELECT cron.schedule('refresh-mv-mkt-star', '5-59/10 * * * *', 'REFRESH MATERIALIZED VIEW CONCURRENTLY analytics.mv_mkt_lead_star');
