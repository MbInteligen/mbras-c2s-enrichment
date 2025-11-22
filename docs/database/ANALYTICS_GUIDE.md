# Analytics & BI Guide 📊

## Overview
We have deployed a new **Analytics Layer** directly in the database to support high-speed dashboards and segmentation without needing external tools.

## 🏗️ Data Structure

### 1. `analytics.mv_mkt_lead_star` (The "Star" View)
This is your main table for dashboards. It combines Leads, People, Campaigns, and Assets into a single row per lead.

**Refresh Rate:** Every 10 minutes.

| Column | Description | Example |
|--------|-------------|---------|
| `lead_id` | Unique ID of the lead | `a1b2...` |
| `lead_name` | Name captured in the form | `João Silva` |
| `party_type` | PF (Person) or PJ (Company) | `PF` |
| `lead_score` | AI Enrichment Score (0-1) | `0.95` |
| `campaign_name` | Google Ads Campaign | `Black Friday 2025` |
| `total_assets_brl` | Est. Real Estate Wealth | `1,500,000.00` |
| `cohort_month` | Month lead was created | `2025-11` |

### 2. `core.mv_party_analytics` (The "Base" View)
Deep dive data on people and companies. Use this if you need specific details like "Mother's Name" or "Company Opening Date".

**Refresh Rate:** Every 10 minutes.

## 🚀 How to Use

### In Metabase / Superset / PowerBI
1.  Connect to the Postgres Database.
2.  Select Schema: `analytics`.
3.  Select Table: `mv_mkt_lead_star`.
4.  **Filter** by `lead_created_at` to see recent data.
5.  **Group By** `campaign_name` or `cohort_month`.

### Common Questions Answered

**Q: Who are my high-net-worth leads from last week?**
A: Filter `total_assets_brl > 1M` and `lead_created_at > 7 days ago`.

**Q: Which campaign brings the most property owners?**
A: Group by `campaign_name` and sum `property_count`.

**Q: Are we enriching leads successfully?**
A: Check `is_enriched = true` rate over `cohort_month`.

## 🔄 Data Freshness
- Data is **NOT real-time**. It is up to **15 minutes delayed**.
- This is intentional to keep the app fast.
- For real-time operational data, look at the CRM, not this Analytics view.

## 🛡️ Audit Trail
We now track all changes to critical data. If a lead's phone number changes, we know:
- **Who** changed it.
- **When** it changed.
- **What** the old value was.
- **What** the new value is.

*Contact the Data Team to access Audit Logs.*
