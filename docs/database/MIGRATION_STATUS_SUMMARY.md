# Migration Status Summary - Quick Reference

**Last Updated:** 2025-11-22 17:30 UTC  
**Overall Progress:** 93% Complete (14 of 15 migrations)

---

## Current State

### ✅ Completed (14 migrations)

| # | Migration | Date | Result |
|---|-----------|------|--------|
| 1-8 | Foundation & Party Model | 2025-11-21/22 | Base schema established |
| 9 | Create party_addresses | 2025-11-22 | 11,530 addresses migrated |
| 10 | Migrate financials to JSONB | 2025-11-22 | 250 records → JSONB |
| 11 | Add party IDs to transactions | 2025-11-22 | Schema updated |
| 12 | Drop entity FK constraints | 2025-11-22 | 0 FKs to entities |
| 13 | Drop split contact tables | 2025-11-22 | 3 tables dropped |
| **14** | **Archive entity_* tables** | **2025-11-22 17:08** | **2.4 GB archived** |

### ⏳ Pending (1 migration)

| # | Migration | When | Action |
|---|-----------|------|--------|
| 15 | Drop archived tables | After 2026-02-20 | Reclaim 2.4 GB |

---

## Key Metrics

**Database:**
- Parties: 1,536,789
- Party_contacts: 2,595,235 (unified)
- Party_addresses: 11,530
- Party_enrichments: 695,310 (250 with financials)

**Archive Schema:**
- Tables: 7 entity_* tables
- Size: 2.4 GB (2,383 MB)
- Status: Read-only, 90-day retention
- Safe to drop: 2026-02-20

**Legacy Dependencies:**
- FK constraints to core.entities: 0 ✅
- entity_* tables in core schema: 0 ✅
- Split contact tables: 0 ✅

---

## Migration 014 Details

**Applied:** 2025-11-22 17:08:12 UTC  
**Result:** 7 tables moved to archive schema

### Archived Tables

| Table | Size | Rows | Replacement |
|-------|------|------|-------------|
| entity_addresses | 319 MB | 11,530 | party_addresses |
| entity_emails | 371 MB | 2,595,091 | party_contacts |
| entity_phones | 724 MB | 2,595,235 | party_contacts |
| entity_profiles | 517 MB | 695,310 | party_enrichments |
| entity_financials | 360 KB | 250 | party_enrichments (JSONB) |
| entity_family_relationships | 259 MB | - | party_relationships |
| entity_relationships | 192 MB | - | party_relationships |

### Monitoring Views

```sql
-- Check when safe to drop
SELECT * FROM archive.safe_to_drop;

-- Monitor archive size
SELECT * FROM archive.archive_size_report;
```

---

## Next Steps

### During 90-Day Retention (Now - 2026-02-20)

**Weekly:**
```sql
-- Verify no archive usage
SELECT * FROM archive.safe_to_drop;

SELECT 
    schemaname, tablename, seq_scan, idx_scan,
    n_tup_ins, n_tup_upd, n_tup_del
FROM pg_stat_user_tables
WHERE schemaname = 'archive';
-- Expected: All modification counts = 0
```

**Monthly:**
- Review application logs for entity_* references (should be none)
- Verify archive size stable (no growth)
- Check production stability

### Before Migration 015 (On/After 2026-02-20)

1. **Verify retention period elapsed:**
   ```sql
   SELECT * FROM archive.safe_to_drop;
   -- All should show "YES - Safe to drop"
   ```

2. **Create final backups (CRITICAL):**
   ```bash
   # Full database backup
   pg_dump "$DB_URL" --format=custom --compress=9 \
     --file="$HOME/Downloads/FINAL-pre-drop-$(date +%Y%m%d).dump"
   
   # Archive schema only
   pg_dump "$DB_URL" --schema=archive --format=plain | \
     gzip -9 > "$HOME/Downloads/archive-only-$(date +%Y%m%d).sql.gz"
   ```

3. **Run migration 015:**
   ```bash
   sqlx migrate run
   # or
   psql "$DB_URL" -f migrations/015_drop_archived_entity_tables.sql
   ```

4. **Verify cleanup:**
   ```sql
   -- Archive schema gone
   SELECT COUNT(*) FROM information_schema.schemata WHERE schema_name = 'archive';
   -- Expected: 0
   
   -- Metadata preserved
   SELECT COUNT(*) FROM public.dropped_tables_log;
   -- Expected: 7
   
   -- Space reclaimed
   SELECT pg_size_pretty(pg_database_size(current_database()));
   -- Expected: ~2.4 GB smaller
   ```

---

## Quick Commands

### Check Migration Status
```sql
SELECT version, description, success, installed_on::date
FROM _sqlx_migrations ORDER BY version;
```

### Verify Archive State
```sql
-- Tables in archive
SELECT tablename, pg_size_pretty(pg_total_relation_size('archive.'||tablename))
FROM pg_tables WHERE schemaname = 'archive' ORDER BY tablename;

-- Retention status
SELECT table_name, safe_to_drop_after, 
       safe_to_drop_after - CURRENT_DATE as days_remaining
FROM archive.archive_metadata ORDER BY safe_to_drop_after;
```

### Check Production Tables
```sql
SELECT 
    'parties' as table_name, COUNT(*) FROM core.parties
UNION ALL
SELECT 'party_contacts', COUNT(*) FROM core.party_contacts
UNION ALL
SELECT 'party_addresses', COUNT(*) FROM core.party_addresses;
```

---

## Rollback (If Needed)

**Migration 014 rollback** (move tables back to core):
```sql
ALTER TABLE archive.entity_addresses SET SCHEMA core;
ALTER TABLE archive.entity_emails SET SCHEMA core;
ALTER TABLE archive.entity_family_relationships SET SCHEMA core;
ALTER TABLE archive.entity_financials SET SCHEMA core;
ALTER TABLE archive.entity_phones SET SCHEMA core;
ALTER TABLE archive.entity_profiles SET SCHEMA core;
ALTER TABLE archive.entity_relationships SET SCHEMA core;
DROP SCHEMA archive CASCADE;
```

**Migration 015 rollback** (restore from backup):
```bash
# Only option - data permanently deleted after M015
pg_restore --dbname="$DB_URL" --schema=archive \
  "$HOME/Downloads/FINAL-pre-drop-YYYYMMDD.dump"
```

---

## Production Status

**Application:** rust-c2s-api  
**URL:** https://mbras-c2s.fly.dev  
**Database:** PostgreSQL 17.5 on Neon.tech  
**Status:** ✅ Healthy

**Last Deployment:** 2025-11-22  
**Migrations Applied:** 1-14  
**Health Check:** https://mbras-c2s.fly.dev/health

---

## Documentation

**Detailed Docs:**
- `PARTY_MODEL_MIGRATION_COMPLETE.md` - Full migration history
- `ARCHIVE_PROCESS.md` - Archive workflow details
- `MIGRATION_EXECUTION_CHECKLIST.md` - Step-by-step guide

**Migration Files:**
- `migrations/001-014/*.sql` - Applied ✅
- `migrations/015_drop_archived_entity_tables.sql` - Pending ⏳

---

**Timeline:**
- Started: 2025-11-21
- Phase 1-6a Complete: 2025-11-22 17:08
- Monitoring Period: 2025-11-22 → 2026-02-20 (90 days)
- Final Cleanup (Migration 015): On/After 2026-02-20
- 100% Complete: After M015 execution

---

**Status:** 93% Complete - Archive created, monitoring in progress  
**Next Milestone:** 2026-02-20 (safe to drop archived tables)
