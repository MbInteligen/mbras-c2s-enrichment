# RML-1092 - Phase 1 CPF Discovery & Core Enrichment (Parity)

## Summary

Feature key: `phase1-cpf-discovery-core-enrichment`

Objective:
- Implement and align Phase 1 behavior in Rust and TS using one contract and one fixture set.
- Reach parity for CPF discovery flow, direct batch enrichment behavior, retry policy, and cron behavior.

## Scope

In scope:
- Work API `name` module as phone Tier 2 fallback (`modulo=name`)
- Work API `mail` module as email Tier 1 (`modulo=mail`)
- CPF mod-11 validation on phone/name/mail results (reject CNPJ and invalid CPF)
- CPF Lookup DuckDB tier (223M) as phone Tier 3
- Phone tier order: Work phone -> Work name -> DuckDB -> Diretrix -> DBase
- Email tier order: Work mail -> Diretrix
- Income multiplier (default `1.9`) for display fields
- Direct batch endpoint parity (`POST /batch/enrich-direct`)
- Retry policy parity (statuses, backoff, max retries)
- Enrichment cron parity (smart intervals + retry processing)

Out of scope:
- Company intelligence (RML-1093+)
- Lead scoring/risk/property/twenty/mcp phases
- Cross-phase schema additions not required by Phase 1 contract

## References

- Upgrade plan: `docs/UPGRADE_PLAN.md`
- Parity protocol: `docs/parity/CROSS_LANGUAGE_PARITY_PROTOCOL.md`
- TS reference:
  - `/Users/ronaldo/Projects/_ATIVO/MBRAS/tools/ts-c2s-api/src/services/cpf-discovery.service.ts`
  - `/Users/ronaldo/Projects/_ATIVO/MBRAS/tools/ts-c2s-api/src/services/work-api.service.ts`
  - `/Users/ronaldo/Projects/_ATIVO/MBRAS/tools/ts-c2s-api/src/services/cpf-lookup.service.ts`
  - `/Users/ronaldo/Projects/_ATIVO/MBRAS/tools/ts-c2s-api/src/routes/batch.ts`
  - `/Users/ronaldo/Projects/_ATIVO/MBRAS/tools/ts-c2s-api/src/jobs/enrichment-cron.ts`
  - `/Users/ronaldo/Projects/_ATIVO/MBRAS/tools/ts-c2s-api/src/services/retry.service.ts`
  - `/Users/ronaldo/Projects/_ATIVO/MBRAS/tools/ts-c2s-api/src/services/db-storage.service.ts`
  - `/Users/ronaldo/Projects/_ATIVO/MBRAS/tools/ts-c2s-api/src/config/index.ts`
  - `/Users/ronaldo/Projects/_ATIVO/MBRAS/tools/ts-c2s-api/docs/CPF_DISCOVERY.md`
- Rust target modules:
  - `src/enrichment.rs`
  - `src/services.rs`
  - `src/handlers.rs`
  - `src/config.rs`
  - `src/main.rs`

## Required Artifacts

- Contract: `docs/parity/specs/phase1-cpf-discovery-core-enrichment.md`
- Fixtures: `docs/parity/fixtures/phase1-cpf-discovery-core-enrichment.json`
- Backport report: `docs/parity/backport_reports/phase1-cpf-discovery-core-enrichment.md`

## Implementation Plan

1. Finalize contract and confirm canonical TS behavior where docs/comments diverge.
2. Add shared fixtures for discovery tiers, mod-11, income multiplier, endpoint results, retry, cron.
3. Ensure TS passes fixtures (baseline).
4. Implement Rust parity for name/mail modules + mod-11 enforcement + tier order.
5. Implement Rust direct endpoint behavior for `POST /batch/enrich-direct`.
6. Implement Rust retry logic and cron loop parity.
7. Run Rust fixture suite and property tests.
8. Backport Rust findings to TS (tests first, then code).
9. Update `docs/UPGRADE_PLAN.md` progress and mark issue gates.

## Gate Checklist (DoD)

- [ ] `spec_ready` - Contract approved
- [ ] `fixtures_shared` - Shared fixtures committed
- [ ] `ts_green` - TS tests green against fixtures
- [ ] `rust_green` - Rust tests green against fixtures
- [ ] `backport_done` - Rust findings applied to TS (or N/A explained)
- [ ] `docs_synced` - Plan/docs/CLAUDE updated where needed
- [ ] `done` - All above complete

## Test Evidence

TS:
- Command(s): `<fill when executed>`
- Result: `<pass/fail + notes>`

Rust:
- Command(s): `<fill when executed>`
- Result: `<pass/fail + notes>`

Cross-parity:
- Fixture diff check: `<fill when executed>`
- Result: `<pass/fail + notes>`

## Rust Mirror Findings

Track every finding and disposition:
- `<finding>`
- `<impact>`
- `<TS backport action>`

## Rollout / Risk

- Risk level: `high` (touches core enrichment rate path)
- Rollback strategy:
  - Feature flag new tier order and cron in Rust config
  - Keep existing endpoint behavior behind fallback path until fixture parity is green
- Monitoring after release:
  - enrichment rate
  - discovery tier hit distribution
  - retry queue growth and max-retry failures

## Notes

- Memora/Engram log IDs: `n/a in this environment`
- Related issues/PRs: `RML-1092`
