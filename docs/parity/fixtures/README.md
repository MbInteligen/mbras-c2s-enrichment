# Parity Fixtures

Create shared fixture JSON files per feature:
- `docs/parity/fixtures/<feature-key>.json`

Recommended sections:
- `valid`
- `invalid`
- `edge`
- `error`

Both TS and Rust tests should consume the same fixture file.
