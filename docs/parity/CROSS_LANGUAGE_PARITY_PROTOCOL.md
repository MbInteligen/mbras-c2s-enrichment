# Cross-Language Parity Protocol (Rust <-> TS)

Date: 2026-02-15
Status: Active

## Goal

Ship `rust-c2s-api` to parity with `ts-c2s-api` while using each implementation to improve the other.

Rules:
- Both codebases remain autonomous and deployable on their own.
- TS can lead feature exploration speed.
- Rust acts as typed mirror/hardening feedback.
- No feature is "done" unless behavior matches in both.

## Core Model

Single source of truth for behavior:
- Parity contract (feature behavior spec)
- Shared fixtures (valid/invalid/edge/error)
- Invariants (domain rules that must hold)

Implementation authority:
- TS defines initial behavior target for missing Rust features.
- Rust may propose stricter/safer behavior when justified.
- Any Rust improvement is backported to TS (with tests first).

## Required Artifacts Per Feature

For each parity feature, create/update:
- Contract: `docs/parity/specs/<feature-key>.md`
- Fixtures: `docs/parity/fixtures/<feature-key>.json`
- Backport report: `docs/parity/backport_reports/<feature-key>.md`
- Linear issue: include gate checklist from template

If `memora` and `engram` are available, log decisions there too.
If unavailable, document decisions in the feature contract and backport report.

## Lifecycle (Rust Mirror Technique)

1. Contract first
- Define input/output behavior, error mapping, edge cases, and non-goals.
- Add explicit compatibility notes (what must stay identical).

2. Shared fixtures
- Encode all known cases in JSON.
- Include `valid`, `invalid`, `edge`, and `error` cases.

3. TS baseline
- Ensure TS passes contract + fixtures.
- If TS behavior is inconsistent, fix TS before porting.

4. Rust implementation
- Implement to the same contract and same fixtures.
- Add Rust type validations, exhaustive matching, and property tests where useful.

5. Mirror hardening
- Record Rust-discovered issues: ambiguity, unsafe defaults, weak validation, perf traps.
- Capture each finding in backport report.

6. TS backport
- Add/adjust TS tests from Rust findings first.
- Then apply code changes to TS to match improved behavior.

7. Cross-language verification
- Both stacks pass shared fixtures.
- Any intentional divergence must be documented in contract with rationale and approval.

8. Close and sync
- Mark Linear gates complete.
- Update `docs/UPGRADE_PLAN.md` progress and issue status.
- Add short note in `CLAUDE.md` if workflow or conventions changed.

## Gate Checklist (Definition of Done)

Mandatory gates for every parity issue:
- `spec_ready`
- `fixtures_shared`
- `ts_green`
- `rust_green`
- `backport_done`
- `docs_synced`
- `done`

No issue may be closed with one-sided completion.

## Test Strategy

Minimum:
- Same fixture file consumed by TS and Rust tests.
- Unit tests for deterministic logic.
- Integration tests for API-level behavior where relevant.
- Property-based tests for normalization/scoring/validation-heavy logic.

Recommended:
- Add a parity CI job that fails when TS and Rust fixture outputs diverge.

## Drift Control

To prevent silent divergence:
- Contract changes require fixture updates in same PR.
- Fixture changes require both TS and Rust test updates in same PR.
- Behavior changes need a backport report entry.

## Linear Execution Rules

For each Linear issue:
- Assign one feature key.
- Track gate checklist explicitly.
- Link contract, fixture, and backport report.
- Move to Done only when all gates are complete.

## Ownership Model

- TS owner: feature semantics and business flow coverage.
- Rust owner: type/validation rigor, safety/perf checks.
- Shared owner: fixtures/contracts and parity CI.

## Suggested Folder Convention

Create these folders as parity work grows:
- `docs/parity/specs/`
- `docs/parity/fixtures/`
- `docs/parity/backport_reports/`

Keep contracts short and test-oriented.
