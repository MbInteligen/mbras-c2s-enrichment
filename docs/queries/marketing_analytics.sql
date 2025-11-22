-- Marketing Analytics Queries
-- Target: analytics.mv_mkt_lead_star
-- Usage: Copy-paste into Metabase/Superset/DBeaver

-- 1. Hot Leads (High Score + Recent)
-- Leads enriched in the last 7 days with score > 0.8
SELECT 
    lead_id, lead_name, email, phone, lead_score, total_assets_brl
FROM analytics.mv_mkt_lead_star
WHERE lead_created_at >= NOW() - INTERVAL '7 days'
  AND lead_score::numeric >= 0.8
ORDER BY lead_score DESC, lead_created_at DESC
LIMIT 50;

-- 2. Segmentation PF by Age & Income (Asset Proxy)
-- Count of PF leads by age group and asset tier
SELECT 
    CASE 
        WHEN age < 30 THEN 'Under 30'
        WHEN age BETWEEN 30 AND 50 THEN '30-50'
        WHEN age > 50 THEN '50+'
        ELSE 'Unknown'
    END as age_group,
    CASE 
        WHEN total_assets_brl > 1000000 THEN 'High Net Worth (>1M)'
        WHEN total_assets_brl > 0 THEN 'Property Owner'
        ELSE 'Non-Owner'
    END as asset_tier,
    count(*) as lead_count
FROM analytics.mv_mkt_lead_star
WHERE party_type = 'PF'
GROUP BY 1, 2
ORDER BY 1, 2;

-- 3. Segmentation PJ by Company Size
SELECT 
    company_size, 
    count(*) as count,
    avg(total_assets_brl)::money as avg_assets
FROM analytics.mv_mkt_lead_star
WHERE party_type = 'PJ'
GROUP BY 1
ORDER BY count(*) DESC;

-- 4. Campaign Performance (Leads & Assets)
SELECT 
    campaign_name,
    count(*) as leads_generated,
    sum(CASE WHEN is_enriched THEN 1 ELSE 0 END) as enriched_leads,
    sum(total_assets_brl)::money as total_pipeline_value
FROM analytics.mv_mkt_lead_star
GROUP BY 1
ORDER BY leads_generated DESC;

-- 5. Real Estate Portfolio Analysis
-- Leads with multiple properties
SELECT 
    lead_name, 
    property_count, 
    total_assets_brl::money
FROM analytics.mv_mkt_lead_star
WHERE property_count > 1
ORDER BY total_assets_brl DESC
LIMIT 20;

-- 6. Lead Quality Distribution
SELECT 
    lead_score,
    count(*) as count
FROM analytics.mv_mkt_lead_star
WHERE lead_score IS NOT NULL
GROUP BY 1
ORDER BY 1 DESC;

-- 7. Enrichment Timeline (Cohorts)
SELECT 
    cohort_month,
    count(*) as total_leads,
    sum(CASE WHEN is_enriched THEN 1 ELSE 0 END) as enriched_count,
    round((sum(CASE WHEN is_enriched THEN 1 ELSE 0 END)::numeric / count(*)) * 100, 1) as enrichment_rate_pct
FROM analytics.mv_mkt_lead_star
GROUP BY 1
ORDER BY 1 DESC;

-- 8. Geographic Distribution (Top Cities)
-- Note: Requires joining back to address if not in Star, 
-- but assuming we might add city to Star later. 
-- For now, using Party data if available or metadata.
SELECT 
    party_name,
    total_assets_brl::money
FROM analytics.mv_mkt_lead_star
WHERE total_assets_brl > 5000000
ORDER BY total_assets_brl DESC;

-- 9. Missing Contact Info (Data Quality)
SELECT 
    count(*) as total,
    sum(CASE WHEN email IS NULL THEN 1 ELSE 0 END) as missing_email,
    sum(CASE WHEN phone IS NULL THEN 1 ELSE 0 END) as missing_phone
FROM analytics.mv_mkt_lead_star;

-- 10. Recent High-Value PJ Leads
SELECT 
    party_name, 
    company_type, 
    lead_created_at
FROM analytics.mv_mkt_lead_star
WHERE party_type = 'PJ'
  AND total_assets_brl > 2000000
  AND lead_created_at >= NOW() - INTERVAL '30 days';
