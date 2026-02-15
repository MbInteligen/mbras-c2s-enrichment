# Migrations — Baseline & Rollback Playbook

**Date:** 2026-02-15
**Database:** Neon PostgreSQL (São Paulo region)
**ORM:** SQLx (compile-time checked queries)

## Migration Inventory

| # | File | Purpose | Reversible? |
|---|------|---------|-------------|
| 001 | `001_hardening_constraints.sql` | Add NOT NULL / CHECK constraints | Yes (ALTER DROP) |
| 002 | `002_create_webhook_events.sql` | Create webhook_events table | Yes (DROP TABLE) |
| 003 | `003_google_ads_leads.sql` | Create google_ads_leads table | Yes (DROP TABLE) |
| 004 | `004_fix_orphaned_data.sql` | Fix orphaned records | No (data mutation) |
| 005 | `005_analytics_mvs.sql` | Create materialized views | Yes (DROP MV) |
| 006 | `006_audit_trail.sql` | Create audit tables | Yes (DROP TABLE) |
| 007 | `007_implement_party_model.sql` | Create party model tables | Yes (DROP TABLE) |
| 008 | `008_party_model_backfill.sql` | Backfill party data | No (data migration) |
| 009 | `009_create_party_addresses.sql` | Create party_addresses table | Yes (DROP TABLE) |
| 010 | `010_migrate_financials_to_jsonb.sql` | Move financials to JSONB | No (schema change + data) |
| 011 | `011_add_party_ids_to_transactions.sql` | Add party_id FK to transactions | Yes (ALTER DROP) |
| 012 | `012_drop_entity_foreign_keys.sql` | Drop entity FK constraints | No (constraint removal) |
| 013 | `013_drop_unused_party_contact_tables.sql` | Drop unused tables | No (data loss) |
| 014 | `014_archive_entity_tables.sql` | Rename entity tables to _archived | Yes (rename back) |
| 015 | `015_drop_archived_entity_tables.sql` | Drop _archived entity tables | No (data loss, ~2.4 GB) |
| 016 | `016_archive_entities_table.sql` | Archive entities table | Yes (rename back) |
| 017 | `017_rebuild_mv_entity_enriched.sql` | Rebuild MV | Yes (DROP + recreate) |
| 018 | `018_drop_app_and_dim_schemas.sql` | Drop app/dim schemas | No (data loss) |
| 019 | `019_webhook_dedup_lock.sql` | UNIQUE index for atomic webhook dedup | Yes (DROP INDEX) |

**Total:** 19 migrations
**Irreversible:** 7 (004, 008, 010, 012, 013, 015, 018)

## Current State

All 18 migrations have been applied. The `_sqlx_migrations` table tracks applied state.

```sql
SELECT version, description, installed_on, success
FROM _sqlx_migrations
ORDER BY version;
```

## Rollback Procedure

### Before running any new migration:

1. **Verify current state:**
   ```sql
   SELECT COUNT(*) FROM _sqlx_migrations WHERE success = true;
   -- Expected: 19
   ```

2. **Take snapshot** (Neon branch):
   ```bash
   # Create a Neon branch as a point-in-time backup
   neonctl branches create --name pre-migration-$(date +%Y%m%d)
   ```

3. **Test in branch first:**
   ```bash
   # Run migration against branch, verify, then apply to main
   DB_URL=$BRANCH_URL sqlx migrate run
   ```

### Rolling back a reversible migration:

```sql
-- 1. Run the reverse SQL
-- 2. Delete the migration record
DELETE FROM _sqlx_migrations WHERE version = <version>;
```

### Rolling back an irreversible migration:

```bash
# Restore from Neon branch
neonctl branches restore <branch-name>
```

## Adding New Migrations

```bash
# Create new migration
sqlx migrate add <description>

# Run migrations
sqlx migrate run

# Check status
sqlx migrate info
```

## Safety Rules

1. Always create a Neon branch before destructive migrations
2. Test migrations against branch database first
3. Never DROP without prior RENAME TO _archived (2-week grace period)
4. Document rollback steps in the migration file header
5. Mark irreversible migrations with `-- IRREVERSIBLE` comment
