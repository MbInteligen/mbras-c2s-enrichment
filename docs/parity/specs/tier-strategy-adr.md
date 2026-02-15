# ADR: Tier Strategy — MimirService Deprecation

**Status:** Accepted
**Date:** 2026-02-15
**Context:** Phase 0 foundation for consolidation

## Decision

Deprecate MimirService from CPF discovery pipeline. Keep it only for IBVI property queries.

## Context

MimirService was a CPF discovery tier that queried the IBVI Mimir API for person data by name. It has been superseded by:

1. **Work API `name` module** — Direct CPF search by name (~2s, more reliable)
2. **Work API `mail` module** — Direct CPF search by email (~2s)
3. **CPF Lookup DuckDB** — 223M records, name-based fallback (~2min)

MimirService adds latency without improving discovery rates beyond these three sources.

## Consequences

### Positive

- Simpler CPF discovery pipeline (fewer tiers to maintain)
- No dependency on IBVI infrastructure for lead enrichment
- Clearer separation: enrichment API (C2S) vs property API (IBVI)

### Negative

- Loses one fallback tier (acceptable given Work API name/mail coverage)

### Neutral

- MimirService remains available for IBVI property queries (`IbviPropertyService`)
- No code deletion needed in TS — service still used for property lookups
- Rust implementation skips Mimir tier entirely

## CPF Discovery Tiers (Post-Decision)

| Tier | Source | Module | Speed |
|------|--------|--------|-------|
| 1 | Work API | phone | ~2s |
| 2 | Work API | name | ~2s |
| 3 | CPF Lookup | DuckDB | ~2min |
| 4 | Diretrix | phone | ~500ms |
| 5 | DBase | phone | ~100ms |

## References

- `ts-c2s-api/src/services/cpf-discovery.service.ts` — Current 5-tier implementation
- `ts-c2s-api/docs/CPF_DISCOVERY.md` — Discovery documentation
