# Rust C2S API — Upgrade Plan v2

**Goal:** Bring rust-c2s-api to feature parity with ts-c2s-api
**Reference:** `ts-c2s-api/docs/RUST_VS_TS_COMPARISON.md`
**Canonical scope:** 106 checkboxes across 13 phases (Phase 0-12)
**MCP Server:** 66 tools (Phase 12, counted as 16 checklist rows)

> **Scope accounting:** Phases 0-11 = 90 feature checkboxes. Phase 12 = 16 MCP rows (66 tools). Grand total = 106 checkboxes.
> The comparison doc lists 68 "Rust missing" categories; this plan expands those into concrete implementation tasks.
> Phase 0 adds 8 new items not in the original comparison (parity contract, metrics, migration, scheduler).

## Scope Accounting

| Phases | Checkboxes | Notes |
|--------|-----------|-------|
| Phase 0 | 8 | Foundation (1 done: rate limiter) |
| Phase 1 | 13 | CPF Discovery |
| Phase 2 | 7 | Company Intelligence |
| Phase 3 | 7 | Lead Scoring |
| Phase 4 | 12 | Alerting \& Monitoring |
| Phase 5 | 9 | Web Intelligence |
| Phase 6 | 4 | Property Intelligence |
| Phase 7 | 7 | CRM Extended |
| Phase 8 | 8 | Twenty CRM |
| Phase 9 | 5 | Reporting |
| Phase 10 | 5 | Photo \& Media |
| Phase 11 | 5 | Infrastructure |
| **Phases 0-11 subtotal** | **90** | |
| Phase 12 | 16 | MCP Server (16 rows → 66 tools) |
| **Grand total** | **106** | 90 features + 66 MCP tools |


---

## Linear Project Tracking

**Project:** Rust C2S API — Feature Parity Upgrade
**Project ID:** `f43df1ed-be9c-4214-b68a-78fef78a9577`
**Team:** RML

| Issue | Phase | Priority | Features |
|-------|-------|----------|----------|
| **RML-1105** | Phase 0: Parity Contract & Foundation | Urgent | 8 |
| **RML-1092** | Phase 1: CPF Discovery & Core Enrichment | Urgent | 13 |
| **RML-1093** | Phase 2: Company Intelligence - Meilisearch 65M | Urgent | 7 |
| **RML-1094** | Phase 3: Lead Scoring & Classification | Urgent | 7 |
| **RML-1095** | Phase 4: Alerting & Monitoring | Medium | 12 |
| **RML-1096** | Phase 5: Web Intelligence & Risk Detection | Medium | 9 |
| **RML-1097** | Phase 6: Property Intelligence - IBVI | Medium | 4 |
| **RML-1098** | Phase 7: CRM Extended - C2S | Medium | 7 |
| **RML-1099** | Phase 8: Twenty CRM Integration | Medium | 8 |
| **RML-1100** | Phase 9: Reporting MD/HTML/PDF | Medium | 5 |
| **RML-1101** | Phase 10: Photo Storage - Cloudflare R2 | Low | 5 |
| **RML-1102** | Phase 11: Infrastructure & Auto-Scaling | Low | 5 |
| **RML-1103** | Phase 12: MCP Server - 66 AI Tools | Low | 66 tools |
| **RML-1104** | Phase 12b: Database Schema Updates | High | -- (inline) |

---

## Phase 0 — Parity Contract & Foundation (RML-1105)

**Priority:** URGENT — Must complete before any Phase 1+ work begins.
**Rationale:** Establishes measurable baselines, migration safety, and distributed coordination so Phase 1-3 KPIs are measurable from day one.

- [ ] **Parity contract document** — One-to-one mapping from comparison doc "Rust missing" rows to phase/checkbox. Published as `docs/PARITY_CONTRACT.md`. Any new feature must trace back to a comparison-doc row or an explicit "new" marker.
- [ ] **Minimal observability (Prometheus)** — `GET /metrics` endpoint with Histogram `enrichment_duration_seconds` (by tier) + labeled counters: `enrichment_requests_total` (status), `cpf_discovery_total` (tier, result), `http_requests_total` (method, route_template, status). Uses `prometheus` + `axum-prometheus` crates. Required to measure the 70% to 92% enrichment rate claim.
- [ ] **Enrichment rate baseline** — Record current enrichment success rate from production logs. Store as `docs/BASELINE_METRICS.md` with date, sample size, and rate. All future phase KPIs reference this.
- [ ] **Migration baseline & rollback playbook** — Audit all 18 existing migrations. Verify `_sqlx_migrations` table is consistent. Document rollback procedure for every future migration. Create `migrations/README.md`.
- [ ] **Distributed scheduler lock** — Replace local `tokio::spawn` in webhook_handler.rs with advisory lock (`pg_try_advisory_lock`) or atomic claim (`UPDATE ... WHERE claimed_at IS NULL`). Prevents duplicate processing if Fly.io runs >1 instance.
- [x] **Re-enable rate limiter** — Rate limiter was disabled for local testing. Done (re-enabled in this session).
- [ ] **Resolve tier strategy** — Current code: DBase(1st) then Mimir(2nd) then Diretrix(3rd, disabled). Planned: Work phone then Work name then DuckDB then Diretrix then DBase. Mimir is out-of-scope. **Decision: deprecate MimirService from discovery, keep for IBVI queries.** Document in ADR.
- [ ] **Party model Phase 6b** — Drop archived entity tables (safe after Feb 20, 2026). Run migration 015. Reclaim ~2.4 GB. Must complete before Phase 12b schema work.

**Acceptance criteria:**
- `GET /metrics` returns Prometheus-formatted counters
- `docs/BASELINE_METRICS.md` exists with enrichment rate + date
- `docs/PARITY_CONTRACT.md` maps every comparison-doc gap to a phase/checkbox
- Advisory lock or atomic claim prevents duplicate webhook processing
- `cargo check` passes with 0 warnings
- `cargo test` passes (22+ tests, excluding pre-existing proptest DDD-90 issue)
- Migration 015 applied and ~2.4 GB reclaimed (if past Feb 20)

---

## Phase 1 — CPF Discovery & Core Enrichment (RML-1092)

**Priority:** URGENT — Directly impacts enrichment rate (baseline ~70%, target 92%)
**Depends on:** Phase 0 (metrics must exist to measure improvement)

- [ ] Work API `name` module (Tier 2 — search CPF by name, max 20 results, score >= 0.7)
- [ ] Work API `mail` module (Email Tier 1 — search CPF by email)
- [ ] CPF mod-11 validation (static `is_valid_cpf()`, reject invalid CPFs from ALL modules)
- [ ] CPF Lookup DuckDB client (Tier 3 — HTTP call to cpf-lookup-api for 223M fallback)
- [ ] Reorder discovery tiers: Work phone(1) then Work name(2) then DuckDB(3) then Diretrix(4) then DBase(5)
- [ ] Email discovery 2 tiers: Work mail(1) then Diretrix(2)
- [ ] Deprecate MimirService from discovery flow (keep code, remove from `find_cpf_via_diretrix`)
- [ ] Income multiplier (x1.9 for display, configurable via env `INCOME_MULTIPLIER`)
- [ ] Batch enrichment endpoint (`POST /batch/enrich-direct` with 4-tier CPF discovery)
- [ ] Enrichment retry service (retry failed/partial leads with exponential backoff)
- [ ] Enrichment cron (background loop: business hours 5min, evening 20min, night 60min)
- [ ] Enrichment status lifecycle (`pending` then `processing` then `completed/partial/failed/basic`)
- [ ] `c2s_leads` table — auto-save webhook leads to PostgreSQL before enrichment

**Acceptance criteria:**
- Enrichment rate >= 85% (measured via `/metrics` counter)
- 5-tier phone discovery + 2-tier email discovery operational
- `POST /batch/enrich-direct` returns `cpfSource`, `matchScore` per lead
- Cron retries failed leads without manual intervention
- All webhook leads persisted even if enrichment fails

---

## Phase 2 — Company Intelligence (RML-1093)

**Priority:** URGENT — Key differentiator for lead qualification

- [ ] Meilisearch company service (65M CNPJs, `filter: socios_cpfs = {cpf}`)
- [ ] CompanySummary model (cargo, totalSocios, participacaoEstimada, qualificacao labels)
- [ ] CNPJ lookup by CPF endpoint
- [ ] Company data persistence (`parties.company_data` JSONB column)
- [ ] C2S message formatting with company data (2-line format, up to 5 companies)
- [ ] Auto-scaling for Meilisearch machine (shared-1x/2GB to shared-8x/16GB, 10min idle)
- [ ] CNPJa/Econodata fallback (web lookup when Meilisearch misses, rate-limited)

**Acceptance criteria:**
- Given a CPF with known companies, company endpoint returns CompanySummary
- Company data persisted as JSONB on `parties.company_data`
- C2S enrichment message includes company section
- Meilisearch auto-scales up before search, down after 10min idle

---

## Phase 3 — Lead Scoring & Classification (RML-1094)

**Priority:** URGENT — Required for CRM routing and seller prioritization

- [ ] Quality score service (0-100: dataCompleteness 30, income 25, location 15, contacts 20, enrichment 10)
- [ ] Income proxy for missing income (company capital + admin role + real estate sector, capped at 25)
- [ ] Tier calculator (S/A/B/C/D based on score thresholds: A=90, B=70, C=50, D=30, F=0)
- [ ] High-value detector (platinum/gold/silver/bronze/risk, uncapped score)
- [ ] Noble neighborhood list (Jardins, Itaim, Leblon, Vila Nova Conceicao, etc.)
- [ ] Notable family detection (Safra, Lemann, Ermirio de Moraes, etc.)
- [ ] Corporate email detection (non-free domain = higher contact validity)

**Acceptance criteria:**
- `score_lead_quality(input)` returns 0-100 with grade and breakdown
- Income proxy fills income bucket when income is missing (no double counting)
- `detect_high_value(criteria)` returns tier + reasons + score
- Noble neighborhoods and notable families produce correct flag/bonus

---

## Phase 4 — Alerting & Monitoring (RML-1095)

**Priority:** MEDIUM — Operational visibility (Prometheus already in Phase 0)

- [ ] Slack webhook alert service
- [ ] Email alert service (Resend REST API via reqwest)
- [ ] High-value lead alerts (async, non-blocking via `tokio::spawn`)
- [ ] Service-down alerts (per-service tracking, composite rate-limit key)
- [ ] Low enrichment rate alerts (threshold < 80%, check every 6 hours)
- [ ] Alert rate-limiting (composite key `type:service`, prevent suppression across services)
- [ ] Extended Prometheus metrics (HTTP latency histogram, API call counters, cache hit rates)
- [ ] Service health tracking (6 services: Work API, Diretrix, DBase, Meilisearch, C2S, CPF Lookup)
- [ ] Enrichment rate monitor (periodic check, exposed via `/stats/enrichment`)
- [ ] Dashboard HTML (server-rendered monitoring page at `/dashboard`)
- [ ] Dashboard session authentication (login/logout, 24h cookies, MBRAS branding)
- [ ] API key auth middleware (for programmatic access)

**Acceptance criteria:**
- High-value lead triggers Slack + email alert within 60s
- Service-down alert fires when any service returns 5+ consecutive errors
- `/dashboard` shows enrichment stats, service health, recent leads
- `/dashboard/login` protects dashboard with session auth
- `/metrics` exposes Prometheus histograms and counters

---

## Phase 5 — Web Intelligence & Risk (RML-1096)

**Priority:** MEDIUM — Deeper lead qualification

- [ ] Google Custom Search service (via `GOOGLE_API_KEY` + `GOOGLE_CSE_ID`)
- [ ] Person search (LinkedIn, business profiles)
- [ ] News search (negative news flagging)
- [ ] Email domain analyzer (company identification, trust score)
- [ ] Risk detector service (criminal, investigation, financial, legal categories)
- [ ] Known risk database (hardcoded flagged individuals, e.g. CPI das Bets)
- [ ] Web insight generator + surname analyzer
- [ ] Name matcher (Levenshtein distance via `strsim` crate)
- [ ] Lead analysis caching (`lead_analyses` table, avoid re-analyzing same lead)

**Acceptance criteria:**
- `analyze_lead(id)` returns tier, discovered info (company/role/LinkedIn), recommendation
- Known risk individuals flagged with severity level
- Domain analysis identifies company from corporate email
- Analysis results cached in `lead_analyses` table, reused for 7 days

---

## Phase 6 — Property Intelligence (RML-1097)

**Priority:** MEDIUM — Real estate-specific value

- [ ] IBVI property service (query `core.property_ownerships` by CPF via IBVI PostgreSQL)
- [ ] Property portfolio summary (total value, count, built area)
- [ ] Property message formatting for C2S
- [ ] IPTU report generator (HTML template via `askama`, PDF via Chrome headless)

**Acceptance criteria:**
- Property endpoint returns portfolio summary for a CPF
- Properties included in C2S enrichment message
- IPTU report generates valid PDF

---

## Phase 7 — CRM Extended (C2S) (RML-1098)

**Priority:** MEDIUM — Operational completeness

- [ ] C2S seller management (list, create, update sellers)
- [ ] C2S tag management (list, create, add tag to lead)
- [ ] C2S lead activities (register calls, meetings, emails, tasks)
- [ ] C2S lead forwarding
- [ ] Queue distribution service (auto-assign, distribute, rebalance leads across sellers)
- [ ] Enrichment status tracking with retry count
- [ ] C2S lead search by phone/email

**Acceptance criteria:**
- All C2S CRUD endpoints functional
- Queue distribution assigns leads round-robin
- Lead activities (call, meeting, email) create correct C2S records

---

## Phase 8 — Twenty CRM Integration (RML-1099)

**Priority:** MEDIUM — Multi-workspace lead routing

- [ ] Twenty service (GraphQL client via reqwest)
- [ ] Lead CRUD (create, update, get)
- [ ] Workspace routing (S/A to WS-SENIOR, B/C/Risk to WS-GENERAL)
- [ ] SLA tracking (S=2h, A=24h, B=48h, C=72h)
- [ ] Lead delegation with expiry (S/A: 7d, others: 14d)
- [ ] Intent signal calculation (high/medium/low from activity patterns)
- [ ] Pipeline stats, broker stats, adoption metrics
- [ ] SLA violation detection + bulk import with deduplication

**Acceptance criteria:**
- Lead creation auto-routes to correct workspace by tier
- SLA violations detected and queryable
- Delegation expiry tracked with auto-cleanup

---

## Phase 9 — Reporting (RML-1100)

**Priority:** MEDIUM — Output for stakeholders

- [ ] Profile report service (Markdown + HTML via `askama` templates)
- [ ] PDF generation (headless Chrome `--print-to-pdf`)
- [ ] Report from CPFs pipeline (lookup then enrich then report)
- [ ] Seller ranking reports (leads by seller, tier distribution)
- [ ] Lead quality reports (HTML with Tailwind CSS, MBRAS branding)

**Acceptance criteria:**
- Report generation endpoint returns HTML report
- PDF generation produces valid file
- Reports include tier badges, KPI cards, company data

---

## Phase 10 — Photo & Media (RML-1101)

**Priority:** LOW — Nice to have

- [ ] Work API photo extraction (base64 from `foto` field)
- [ ] Cloudflare R2 upload service (`aws-sdk-s3` crate, S3 compatible)
- [ ] Signed URL generation (7-day expiry)
- [ ] Photo URL persistence (`parties.photo_url`)
- [ ] Fire-and-forget upload (non-blocking, never blocks enrichment)

**Acceptance criteria:**
- Photos uploaded to R2, URL stored on party
- Signed URLs work for 7 days
- Failed photo upload does not affect enrichment

---

## Phase 11 — Infrastructure & Scaling (RML-1102)

**Priority:** LOW — Cost optimization

- [ ] Fly.io auto-scale service (generic multi-machine profiles via Machines API)
- [ ] CPF Lookup auto-scaling (256MB to 16GB, 5min idle timeout)
- [ ] Meilisearch auto-scaling profile ownership (2GB to 16GB, 10min idle)
- [ ] Scale-down timer per profile (cancel on new request, fire after idle period)
- [ ] Cross-org token support (per-machine Fly.io tokens for scaling across orgs)

> **Note:** Meilisearch auto-scaling is **implemented** in Phase 2 (calling `fly_scale.scale_up("meilisearch")`) but **profile definition and maintenance** lives in Phase 11. This avoids duplication while keeping Phase 2 focused on company intelligence.

**Acceptance criteria:**
- Machines scale up before operations, down after idle
- Monthly cost stays under $15 for both machines combined
- Scale operations logged with timing

---

## Phase 12 — MCP Server (RML-1103)

**Priority:** LOW — AI assistant integration (depends on Rust MCP SDK maturity)

- [ ] MCP server entry point (stdio transport)
- [ ] Enrichment tools (3): enrich_lead, enrich_bulk, retry_failed
- [ ] Discovery tools (5): find_and_save_person, discover_cpf, lookup_cpf, search_cpf_by_name, validate_cpf
- [ ] Lead tools (3): get_lead, list_leads, get_c2s_lead_status
- [ ] Stats tools (4): get_enrichment_stats, get_service_health, get_enrichment_rate, get_enrichment_health
- [ ] Property tools (3): get_properties_by_cpf, get_property_summary, format_property_message
- [ ] Report tools (3): generate_profile_report, generate_report_from_cpfs, generate_report_pdf
- [ ] Analysis tools (6): analyze_lead, get_lead_analysis, check_lead_alert, score_lead_quality, assess_risk, quick_risk_check
- [ ] C2S tools (9): fetch_c2s_leads, get_c2s_sellers, send_c2s_message, forward_c2s_lead, search_c2s_by_phone, search_c2s_by_email, mark_c2s_interacted, get_c2s_tags, add_c2s_lead_tag
- [ ] Domain tools (3): analyze_email_domain, get_domain_trust_score, identify_company_from_email
- [ ] Company tools (7): lookup_cnpj, find_companies_by_name, analyze_company_portfolio, find_companies_by_cpf, get_company_by_cnpj, search_companies, format_companies_message
- [ ] Tier tools (2): calculate_lead_tier, get_tier_recommendation
- [ ] Search tools (5): search_web, search_person, search_news, generate_web_insights, analyze_lead_name
- [ ] Twenty tools (13): all CRUD + analytics + workflow tools
- [ ] MCP resources (3): enrichment://stats, enrichment://health, enrichment://recent
- [ ] **Total: 66 tools + 3 resources**

**Acceptance criteria:**
- MCP server runs via stdio
- All 66 tools respond with correct JSON schema
- MCP resources return real-time data

---

## Phase 12b — Database Schema Updates (RML-1104)

**Priority:** Runs inline with each phase — NOT a standalone deliverable.

Schema changes are executed as part of their parent phase. This section is a **registry** of all migrations.

| Migration | Parent Phase | Table/Column | Type |
|-----------|-------------|--------------|------|
| 019 | Phase 0 | Advisory lock pattern | Coordinator |
| 020 | Phase 1 | analytics.c2s_leads | New table |
| 021 | Phase 2 | parties.company_data | JSONB column |
| 022 | Phase 5 | analytics.lead_analyses | New table |
| 023 | Phase 5 | analytics.party_insights | New table |
| 024 | Phase 10 | parties.photo_url | VARCHAR column |

**Rules:**
- Every migration has a rollback SQL in comments
- Migrations run in CI before deploy
- No migration may drop data without 90-day archive period

---

## Execution Order

```
Phase 0  (Foundation)           ****         <- MUST complete first
Phase 1  (CPF Discovery)       ************  <- Biggest ROI
Phase 2  (Companies)           ************
Phase 3  (Scoring)             ************
  Checkpoint: enrichment rate >= 85%, scoring operational
Phase 4  (Alerts/Monitoring)   ********
Phase 5  (Web Intel/Risk)      ********
Phase 6  (Properties)          ******
Phase 7  (C2S Extended)        ******
Phase 8  (Twenty CRM)          ******
  Checkpoint: all CRM integrations functional
Phase 9  (Reporting)           ****
Phase 10 (Photos)              ***
Phase 11 (Infrastructure)      ***
Phase 12 (MCP Server)          **********    <- Last, depends on SDK
```

---

## Tier Strategy (Resolved)

### Current code (enrichment.rs)

```
Phone: DBase(1st) -> Mimir(2nd) -> Diretrix(3rd, DISABLED)
Email: Diretrix (DISABLED)
```

### Target (after Phase 0 + Phase 1)

```
Phone: Work API phone(1) -> Work API name(2) -> DuckDB 223M(3) -> Diretrix(4) -> DBase(5)
Email: Work API mail(1) -> Diretrix(2)
```

### Migration path

1. **Phase 0:** Deprecate MimirService from discovery flow. Remove from `find_cpf_via_diretrix()`. Keep `MimirService` code (used by IBVI property queries in Phase 6). Write ADR.
2. **Phase 1:** Add Work API name/mail modules. Add DuckDB HTTP client. Reorder tiers. Re-enable Diretrix as Tier 4.
3. **Config:** `mimir_token` stays in Config (used for IBVI, not CPF discovery). Add `CPF_LOOKUP_API_URL` env var.

---

## Crate Candidates

| Need | Crate | Version | Phase |
|------|-------|---------|-------|
| Prometheus metrics | prometheus + axum-prometheus | Latest | 0 |
| Meilisearch client | meilisearch-sdk | ^0.21 | 2 |
| String similarity | strsim | ^0.11 | 5 |
| Slack webhooks | reqwest (raw POST) | 0.12 | 4 |
| Email (Resend) | reqwest (raw POST) | 0.12 | 4 |
| S3/R2 uploads | aws-sdk-s3 | Latest | 10 |
| PDF generation | headless_chrome or shell | -- | 9 |
| Cron scheduling | tokio-cron-scheduler | Latest | 1 |
| MCP server | mcp-sdk or custom stdio | Latest | 12 |
| HTML templating | askama | Latest | 9 |
| Advisory lock | sqlx (built-in) | 0.8 | 0 |

---

## Out of Scope

- **Mimir Azure as CPF discovery fallback** — Deprecated. MimirService kept only for IBVI property queries (Phase 6).
- **100+ scripts/** — Stay in TS/Python, not ported to Rust.
- **Swagger UI** — Rust already has it (utoipa).
- **Circuit breaker** — Rust already has it (failsafe).
- **SHA-256 cache validation** — Rust already has it.
- **Property-based tests** — Rust already has it (proptest).
- **CNPJa/Econodata as primary source** — Only as fallback when Meilisearch misses (Phase 2).

---

## KPIs

| Phase | KPI | Current | Target | How Measured |
|-------|-----|---------|--------|--------------|
| 0 | Baseline documented | -- | Documented | docs/BASELINE_METRICS.md |
| 1 | Enrichment rate | ~70% | >= 85% | enrichment_success / enrichment_total counter |
| 1-3 | Enrichment rate | >= 85% | >= 92% | Same counter, after scoring + companies |
| 4 | Alert latency | -- | < 60s | Time from high-value detection to Slack message |
| 4 | Dashboard uptime | 0% | 100% | /dashboard returns 200 |
| All | Test count | 22 | 50+ | cargo test output |
| All | Cargo warnings | 0 | 0 | cargo check output |

---

**Canonical feature count:** 90 features + 66 MCP tools = 156 total deliverables
**Estimated new code:** ~18,000-22,000 lines
**Final codebase:** ~25,000-29,000 lines

---

**Created:** February 14, 2026
**Updated:** February 15, 2026 (v2 — Phase 0 added, scope reconciled, acceptance criteria, tier strategy resolved)
**Linear Issues:** RML-1092 to RML-1105
