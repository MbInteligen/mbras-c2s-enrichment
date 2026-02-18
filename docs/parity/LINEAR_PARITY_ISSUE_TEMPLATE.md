# Linear Issue Template - Cross-Language Parity

Use this body for parity issues in the Rust upgrade project.

---

## Summary

Feature key: `<phase>-<feature-key>`

Objective:
- Implement/align this feature in Rust and TS with shared behavior contract and fixtures.

## Scope

In scope:
- `<item>`
- `<item>`

Out of scope:
- `<item>`

## References

- Upgrade plan entry: `docs/UPGRADE_PLAN.md`
- Parity protocol: `docs/parity/CROSS_LANGUAGE_PARITY_PROTOCOL.md`
- TS reference implementation: `<path-or-url>`
- Rust target module(s): `<path>`

## Required Artifacts

- Contract: `docs/parity/specs/<feature-key>.md`
- Fixtures: `docs/parity/fixtures/<feature-key>.json`
- Backport report: `docs/parity/backport_reports/<feature-key>.md`

## Implementation Plan

1. Finalize contract and edge cases
2. Add shared fixtures
3. Ensure TS passes fixtures
4. Implement Rust and pass fixtures
5. Backport Rust findings to TS (tests first)
6. Sync docs and close

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
- Command(s): `<cmd>`
- Result: `<pass/fail + notes>`

Rust:
- Command(s): `<cmd>`
- Result: `<pass/fail + notes>`

Cross-parity:
- Fixture diff check: `<cmd or process>`
- Result: `<pass/fail + notes>`

## Rust Mirror Findings

List every relevant finding:
- `<finding>`
- `<impact>`
- `<TS backport action>`

## Rollout / Risk

- Risk level: `<low|medium|high>`
- Rollback strategy: `<short rollback plan>`
- Monitoring after release: `<metrics/alerts>`

## Notes

- Memora/Engram log IDs: `<ids-or-n/a>`
- Related issues/PRs: `<links>`

