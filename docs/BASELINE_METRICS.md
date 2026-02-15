# Enrichment Rate Baseline

## Purpose

Reproducible enrichment rate measurement for tracking consolidation progress.

## Measurement Query

```sql
-- Window: last 30 days from measurement date
-- Timezone: UTC
-- Database: analytics DB (prod) — connect via $DB_URL
-- Sample: all c2s_leads received in window
SELECT
  COUNT(*) AS total,
  COUNT(*) FILTER (WHERE enrichment_status = 'completed') AS completed,
  COUNT(*) FILTER (WHERE enrichment_status IN ('partial', 'basic')) AS partial,
  COUNT(*) FILTER (WHERE enrichment_status IN ('unenriched', 'failed')) AS failed,
  ROUND(100.0 * COUNT(*) FILTER (WHERE enrichment_status = 'completed') / COUNT(*), 1) AS rate_pct
FROM analytics.c2s_leads
WHERE received_at >= NOW() - INTERVAL '30 days';
```

## Recording Format

Each measurement should record:

| Field | Format | Example |
|-------|--------|---------|
| Date | ISO 8601 | 2026-02-15 |
| Timezone | IANA | UTC |
| Sample size | integer | 1,200 |
| Completed | integer | 912 |
| Partial | integer | 180 |
| Failed | integer | 108 |
| Rate | percentage | 76.0% |

## Connection

Use `$DB_URL` environment variable. Do **not** commit connection strings.

```bash
psql "$DB_URL" -f docs/baseline-query.sql
```

## Measurement Schedule

- Before starting each consolidation phase
- After completing each phase
- Monthly during active development
