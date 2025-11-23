# Repository Guidelines

## Project Structure & Module Organization
- Core code in `src/` (`main.rs` bootstraps Axum router; `config.rs` handles env loading; `db.rs`/`db_storage.rs` wrap SQLx; `handlers.rs`/`webhook_handler.rs` expose HTTP endpoints; `services.rs`, `gateway_client.rs`, `google_ads_handler.rs` integrate external APIs; `models.rs`/`google_ads_models.rs`/`webhook_models.rs` define payloads; `enrichment.rs` contains enrichment flow).
- All documentation and project resources unified in `docs/`:
  - `docs/adr/` — Architecture Decision Records
  - `docs/architecture/` — System design documents
  - `docs/database/` — Database documentation + schema examples (JSON + Rust examples)
  - `docs/deployment/` — Deployment guides and checklists
  - `docs/integrations/` — External API documentation
  - `docs/queries/` — SQL query examples
  - `docs/schemas/` — Database schema files
  - `docs/scripts/` — All utility scripts (data, deployment, testing)
  - `docs/security/` — Security checklists and guides
  - `docs/session-notes/` — Development session summaries
  - `docs/testing/` — Test documentation
- Test suite in `tests/` (k6 load/smoke tests); container assets in `Dockerfile`, `docker-compose*.yml`; Fly.io config in `fly.toml`.

## Build, Test, and Development Commands
- `cargo build` — compile the service; add `--release` for deploy parity.
- `cargo run` — start the API locally (loads `.env` via dotenv).
- `cargo fmt` / `cargo clippy --all-targets --all-features` — enforce style and lints.
- `cargo test` — run Rust unit/integration tests.
- `./test-local.sh [BASE_URL] [LEAD_ID]` — full happy-path API check against local or remote.
- `./test-docker.sh` — spin up docker-compose test stack and run integration tests.
- `k6 run tests/{smoke-test.js,load-test.js}` — smoke/load validation; set `BASE_URL` env for non-local targets.

## Coding Style & Naming Conventions
- Rust 2021; default to 4-space indentation and idiomatic ownership/borrowing; prefer `?` for error propagation and `anyhow::Result` for handlers.
- Run `cargo fmt` before commits; keep imports organized and avoid unused code caught by Clippy.
- File/module names in `snake_case`; types and traits in `PascalCase`; functions/locals in `snake_case`; constants in `SCREAMING_SNAKE_CASE`.
- Use `tracing::{info, warn, error, instrument}` for observability; keep log fields structured (IDs, CPF, lead_id).

## Testing Guidelines
- Place new Rust tests inline with modules or in dedicated `src/**/tests` mods; name tests after behavior (e.g., `process_lead_returns_200`).
- For endpoint changes, add/adjust curl flows in `scripts/` and rerun `./test-local.sh`; mirror scenarios in k6 scripts when load characteristics change.
- Run `cargo test && ./test-local.sh` before PRs; for schema changes, also run `./test-docker.sh` to validate migrations.

## Commit & Pull Request Guidelines
- Follow Conventional Commit style seen in history (`feat:`, `fix:`, `refactor:`). Scope prefix is optional but helpful (`feat: add cpf caching`).
- PRs should include: summary of behavior change, affected endpoints/modules, test evidence (`cargo test`, `./test-local.sh`, k6 if relevant), and links to issues/requirements. Add example curl/k6 flags when altering request/response shapes.
- Keep diffs small and focused; mention migration or config impacts explicitly and include updated `.env.example` or `google-ads.yaml.example` snippets when required.

## Security & Configuration Tips
- Never commit secrets; copy `.env.example` to `.env` and keep tokens/DB URLs local or in Fly.io secrets. Update `.env.example` and docs when adding required vars.
- Rotate shared credentials in `google-ads.yaml` via secret storage; treat `temp_data/` as non-production scratch.
- Validate database connectivity with `sqlx migrate run` before tests; prefer `sslmode=require` in Postgres URLs (Neon default).
