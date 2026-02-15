# Parity Contract — Rust C2S API Feature Parity

**Date:** 2026-02-15
**Reference:** `ts-c2s-api/docs/RUST_VS_TS_COMPARISON.md`
**Tracking:** Linear project IBVI, issues IBVI-341 through IBVI-353

## Purpose

One-to-one mapping from every "Rust missing" row in the comparison document to a phase and checkbox in `docs/UPGRADE_PLAN.md`. Any new feature must trace back to a comparison-doc row or carry an explicit "new" marker.

---

## Phase 0: Foundation (IBVI-352) — 8 items

| # | Feature | Comparison Row | Status |
|---|---------|---------------|--------|
| 0.1 | Parity contract document | new | Done |
| 0.2 | Prometheus /metrics endpoint | "No Prometheus metrics" | Done |
| 0.3 | Enrichment rate baseline | new | Done |
| 0.4 | Migration baseline & rollback playbook | new | Pending |
| 0.5 | Distributed scheduler lock | new | Pending |
| 0.6 | Re-enable rate limiter | new | Done |
| 0.7 | Tier strategy ADR (deprecate Mimir) | "DBase(1st) then Mimir" | Done |
| 0.8 | Party model Phase 6b (drop archived tables) | new | Pending |

## Phase 1: CPF Discovery & Core Enrichment (IBVI-341) — 13 items

| # | Feature | Comparison Row | Status |
|---|---------|---------------|--------|
| 1.1 | Work API `name` module (Tier 2) | "Missing: Work API name module" | Pending |
| 1.2 | Work API `mail` module (Email Tier 1) | "Missing: Work API mail module" | Pending |
| 1.3 | CPF mod-11 validation | "Missing: CPF mod-11 validation" | Pending |
| 1.4 | CPF Lookup DuckDB client (Tier 3) | "Missing: DuckDB 223M lookup" | Pending |
| 1.5 | Reorder discovery tiers (5-tier) | "Missing: 5-tier fallback" | Pending |
| 1.6 | Email discovery 2 tiers | "Missing: email discovery" | Pending |
| 1.7 | Deprecate MimirService from discovery | "DBase(1st) then Mimir" | Pending |
| 1.8 | Income multiplier (x1.9) | "Missing: income multiplier" | Pending |
| 1.9 | Batch enrichment endpoint | "Missing: batch endpoint" | Pending |
| 1.10 | Enrichment retry service | "Missing: retry service" | Pending |
| 1.11 | Enrichment cron | "Missing: cron retry" | Pending |
| 1.12 | Enrichment status lifecycle | "Missing: status lifecycle" | Pending |
| 1.13 | `c2s_leads` table (auto-save) | "Missing: webhook lead persistence" | Pending |

## Phase 2: Company Intelligence (IBVI-342) — 7 items

| # | Feature | Comparison Row | Status |
|---|---------|---------------|--------|
| 2.1 | Meilisearch company service (65M CNPJs) | "Missing: Meilisearch 65M" | Pending |
| 2.2 | CompanySummary model | "Missing: company summary" | Pending |
| 2.3 | CNPJ lookup by CPF endpoint | "Missing: CNPJ lookups" | Pending |
| 2.4 | Company data persistence (JSONB) | "Missing: company persistence" | Pending |
| 2.5 | Qualificacao labels (RF codes) | "Missing: company summary" | Pending |
| 2.6 | Company message format for C2S | "Missing: company summary" | Pending |
| 2.7 | Auto-scaling for Meilisearch | "Missing: auto-scaling" | Pending |

## Phase 3: Lead Scoring & Classification (IBVI-353) — 7 items

| # | Feature | Comparison Row | Status |
|---|---------|---------------|--------|
| 3.1 | Lead quality score (0-100, 5 buckets) | "Missing: quality scores" | Done |
| 3.2 | High-value detector | "Missing: high-value detection" | Done |
| 3.3 | Tier calculator (platinum-risk) | "Missing: tier classification" | Done |
| 3.4 | Noble neighborhood lookup | "Missing: quality scores" | Done |
| 3.5 | Notable family detection | "Missing: quality scores" | Done |
| 3.6 | Shared parity fixtures (15 cases) | new | Done |
| 3.7 | Parity scripts (sync, hash, runner) | new | Done |

## Phase 4: Alerting & Monitoring (IBVI-343) — 12 items

| # | Feature | Comparison Row | Status |
|---|---------|---------------|--------|
| 4.1 | Alert service (Slack webhook) | "Missing: Slack alerts" | Pending |
| 4.2 | Email alerts (Resend API) | "Missing: email alerts" | Pending |
| 4.3 | High-value lead alerts | "Missing: high-value alerts" | Pending |
| 4.4 | Enrichment monitor service | "Missing: enrichment monitor" | Pending |
| 4.5 | Service health tracking | "Missing: service health" | Pending |
| 4.6 | HTML dashboard | "Missing: HTML dashboard" | Pending |
| 4.7 | Dashboard authentication | "Missing: dashboard auth" | Pending |
| 4.8 | Low enrichment rate alerts | "Missing: enrichment monitor" | Pending |
| 4.9 | Max retries alert | "Missing: enrichment monitor" | Pending |
| 4.10 | Service down alert | "Missing: service health" | Pending |
| 4.11 | High error rate alert | "Missing: service health" | Pending |
| 4.12 | Alert rate limiting | "Missing: Slack alerts" | Pending |

## Phase 5: Web Intelligence & Risk Detection (IBVI-344) — 9 items

| # | Feature | Comparison Row | Status |
|---|---------|---------------|--------|
| 5.1 | Google Custom Search service | "Missing: Google search" | Pending |
| 5.2 | Domain analyzer service | "Missing: domain analysis" | Pending |
| 5.3 | Risk detector service | "Missing: risk detection" | Pending |
| 5.4 | Web insight service | "Missing: web insights" | Pending |
| 5.5 | Lead analysis service | "Missing: deep lead analysis" | Pending |
| 5.6 | Surname analyzer (web context) | "Missing: web insights" | Pending |
| 5.7 | Negative news search | "Missing: negative news" | Pending |
| 5.8 | LinkedIn profile search | "Missing: LinkedIn search" | Pending |
| 5.9 | Known risks database | "Missing: risk detection" | Pending |

## Phase 6: Property Intelligence (IBVI-345) — 4 items

| # | Feature | Comparison Row | Status |
|---|---------|---------------|--------|
| 6.1 | IBVI property service | "Missing: property ownership" | Pending |
| 6.2 | Property summary (aggregated) | "Missing: portfolio summary" | Pending |
| 6.3 | Property message format | "Missing: property message" | Pending |
| 6.4 | IPTU report generator | "Missing: IPTU reports" | Pending |

## Phase 7: CRM Extended (IBVI-346) — 7 items

| # | Feature | Comparison Row | Status |
|---|---------|---------------|--------|
| 7.1 | C2S sellers CRUD | "Missing: seller management" | Pending |
| 7.2 | C2S tags management | "Missing: tags" | Pending |
| 7.3 | C2S activities (notes, calls, emails) | "Missing: activities" | Pending |
| 7.4 | Queue distribution | "Missing: queue distribution" | Pending |
| 7.5 | Lead forwarding | "Missing: lead forwarding" | Pending |
| 7.6 | C2S lead search (phone/email) | "Missing: C2S search" | Pending |
| 7.7 | Mark lead as interacted | "Missing: C2S interaction" | Pending |

## Phase 8: Twenty CRM Integration (IBVI-347) — 8 items

| # | Feature | Comparison Row | Status |
|---|---------|---------------|--------|
| 8.1 | Twenty service (GraphQL client) | "Missing: Twenty CRM" | Pending |
| 8.2 | Workspace routing (OPS/SENIOR/GENERAL) | "Missing: Twenty CRM" | Pending |
| 8.3 | Lead tier SLA enforcement | "Missing: Twenty CRM" | Pending |
| 8.4 | Delegation system with expiry | "Missing: Twenty CRM" | Pending |
| 8.5 | Intent signal calculation | "Missing: Twenty CRM" | Pending |
| 8.6 | Bulk import with dedup | "Missing: Twenty CRM" | Pending |
| 8.7 | Pipeline & broker stats | "Missing: Twenty CRM" | Pending |
| 8.8 | SLA violation detection | "Missing: Twenty CRM" | Pending |

## Phase 9: Reporting (IBVI-348) — 5 items

| # | Feature | Comparison Row | Status |
|---|---------|---------------|--------|
| 9.1 | Profile report (Markdown) | "Missing: reports" | Pending |
| 9.2 | Profile report (HTML) | "Missing: reports" | Pending |
| 9.3 | PDF generation | "Missing: PDF generation" | Pending |
| 9.4 | Report from CPFs pipeline | "Missing: reports" | Pending |
| 9.5 | Seller ranking reports | "Missing: seller rankings" | Pending |

## Phase 10: Photo Storage (IBVI-349) — 5 items

| # | Feature | Comparison Row | Status |
|---|---------|---------------|--------|
| 10.1 | Cloudflare R2 service | "Missing: R2 integration" | Pending |
| 10.2 | Photo extraction from Work API | "Missing: R2 integration" | Pending |
| 10.3 | Signed URL generation (7-day expiry) | "Missing: signed URLs" | Pending |
| 10.4 | Fire-and-forget upload | "Missing: R2 integration" | Pending |
| 10.5 | Photo URL persistence | "Missing: R2 integration" | Pending |

## Phase 11: Infrastructure & Auto-Scaling (IBVI-350) — 5 items

| # | Feature | Comparison Row | Status |
|---|---------|---------------|--------|
| 11.1 | Fly.io auto-scaling service | "Missing: auto-scaling" | Pending |
| 11.2 | CPF Lookup machine profile | "Missing: auto-scaling" | Pending |
| 11.3 | Meilisearch machine profile | "Missing: auto-scaling" | Pending |
| 11.4 | Scale-down timers | "Missing: auto-scaling" | Pending |
| 11.5 | Cost optimization profiles | "Missing: auto-scaling" | Pending |

## Phase 12: MCP Server (IBVI-351) — 16 rows (66 tools)

| # | Category | Tools | Comparison Row | Status |
|---|----------|-------|---------------|--------|
| 12.1 | Enrichment | 3 | "Missing: all 66 MCP tools" | Pending |
| 12.2 | Discovery | 5 | "" | Pending |
| 12.3 | Leads | 3 | "" | Pending |
| 12.4 | Stats | 4 | "" | Pending |
| 12.5 | Property | 3 | "" | Pending |
| 12.6 | Reports | 3 | "" | Pending |
| 12.7 | Analysis | 6 | "" | Pending |
| 12.8 | C2S CRM | 9 | "" | Pending |
| 12.9 | Domain | 3 | "" | Pending |
| 12.10 | Companies | 7 | "" | Pending |
| 12.11 | Tier | 2 | "" | Pending |
| 12.12 | Search | 5 | "" | Pending |
| 12.13 | Twenty | 13 | "" | Pending |
| 12.14 | Monitoring | 2 | "" | Pending |
| 12.15 | Resources | 3 | "" | Pending |
| 12.16 | Server setup | 1 | "" | Pending |

---

## Summary

| Phase | Items | Done | Remaining |
|-------|-------|------|-----------|
| 0 | 8 | 5 | 3 |
| 1 | 13 | 0 | 13 |
| 2 | 7 | 0 | 7 |
| 3 | 7 | 7 | 0 |
| 4 | 12 | 0 | 12 |
| 5 | 9 | 0 | 9 |
| 6 | 4 | 0 | 4 |
| 7 | 7 | 0 | 7 |
| 8 | 8 | 0 | 8 |
| 9 | 5 | 0 | 5 |
| 10 | 5 | 0 | 5 |
| 11 | 5 | 0 | 5 |
| 12 | 16 (66 tools) | 0 | 16 |
| **Total** | **106** | **12** | **94** |

Progress: **12/106 (11.3%)**
