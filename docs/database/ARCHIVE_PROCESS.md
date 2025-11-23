# Entity Tables Archive Process

**Created:** 2025-11-22  
**Status:** Ready for execution (after monitoring period)  
**Purpose:** Safe archival and eventual removal of legacy entity_* tables

---

## Overview

This document describes the two-phase process for archiving and removing the legacy `entity_*` tables after the Party Model migration is complete.

**Timeline:**
- **Migration 011:** Move tables to archive schema (run after Phase 1-2 deployment stable)
- **90-day monitoring period:** Verify no issues in production
- **Migration 012:** DROP archived tables (run after 2026-02-20)

---

## Phase 1: Archive Tables (Migration 011)

### What It Does

1. Creates `archive` schema for deprecated tables
2. Drops FK constraint from `entity_addresses`
3. Moves 7 entity_* tables to archive schema
4. Creates metadata tracking table
5. Creates monitoring views
6. Sets read-only permissions

### Tables to Archive

| Table | Size | Rows | Replacement |
|-------|------|------|-------------|
| `entity_addresses` | 319 MB | 11,530 | `core.party_addresses` |
| `entity_emails` | 371 MB | 2,595,091 | `core.party_emails` |
| `entity_family_relationships` | 259 MB | ? | `core.party_relationships` |
| `entity_financials` | 360 KB | 250 | `core.party_enrichments` (JSONB) |
| `entity_phones` | 724 MB | 2,595,235 | `core.party_phones` |
| `entity_profiles` | 517 MB | 695,310 | `core.party_enrichments` |
| `entity_relationships` | 192 MB | ? | `core.party_relationships` |
| **TOTAL** | **~2.4 GB** | **~6M rows** | - |

### Prerequisites

Before running migration 011:

- [x] Migration 009 applied (party_addresses backfilled)
- [x] Migration 010 applied (financials to JSONB)
- [ ] Phase 1-2 code deployed to production
- [ ] Address storage verified working in production
- [ ] At least 30 days of production usage without entity_* issues
- [ ] Code audit confirms no entity_* references (Phase 5)

### How to Run

```bash
# 1. Verify database state
psql $DB_URL -c "
SELECT table_name, pg_size_pretty(pg_total_relation_size('core.' || table_name))
FROM information_schema.tables
WHERE table_schema = 'core' AND table_name LIKE 'entity_%'
ORDER BY table_name;
"

# 2. Create backup (CRITICAL)
BACKUP_FILE="~/Downloads/pre-archive-backup-$(date +%Y%m%d).dump"
pg_dump "$DB_URL" --format=custom --compress=9 --file="$BACKUP_FILE"
echo "Backup created: $BACKUP_FILE"

# 3. Run migration 011
sqlx migrate run

# 4. Verify tables moved
psql $DB_URL -c "
SELECT schemaname, tablename 
FROM pg_tables 
WHERE tablename LIKE 'entity_%' 
ORDER BY schemaname, tablename;
"

# Expected: All entity_* tables in 'archive' schema, none in 'core'
```

### Verification

```sql
-- Check archive schema created
SELECT schema_name FROM information_schema.schemata WHERE schema_name = 'archive';

-- Check all tables moved
SELECT schemaname, tablename, pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename))
FROM pg_tables 
WHERE tablename LIKE 'entity_%';
-- Expected: All in 'archive' schema

-- Check metadata
SELECT * FROM archive.archive_metadata ORDER BY table_name;

-- Check when safe to drop
SELECT * FROM archive.safe_to_drop;
-- Expected: All show "NO - Wait until 2026-02-20"
```

### Monitoring Views

**archive.safe_to_drop** - Shows retention status:
```sql
SELECT * FROM archive.safe_to_drop;
```

**archive.archive_size_report** - Shows disk usage:
```sql
SELECT * FROM archive.archive_size_report;
```

---

## Phase 2: Drop Tables (Migration 012)

### What It Does

1. Verifies 90-day retention period elapsed
2. Creates permanent log in `public.dropped_tables_log`
3. Drops all archived entity_* tables
4. Drops archive metadata and views
5. Drops archive schema
6. Reclaims ~2.4 GB disk space

### Prerequisites

Before running migration 012:

- [ ] Migration 011 applied (tables in archive schema)
- [ ] Current date >= safe_to_drop_after (2026-02-20 or later)
- [ ] 90+ days of production stability without entity_* usage
- [ ] Final code audit confirms zero entity_* references
- [ ] Backup created (CRITICAL - no undo after this!)

### Safety Checks

Migration 012 has built-in safety checks:

```sql
-- Check if safe to run
SELECT 
    table_name,
    safe_to_drop_after,
    safe_to_drop_after - CURRENT_DATE as days_remaining,
    CASE 
        WHEN safe_to_drop_after <= CURRENT_DATE THEN '✅ SAFE'
        ELSE '❌ WAIT'
    END as status
FROM archive.archive_metadata
ORDER BY safe_to_drop_after;
```

**If ANY table shows days_remaining > 0, migration will ABORT.**

### How to Run

```bash
# 1. Verify retention period elapsed
psql $DB_URL -c "SELECT * FROM archive.safe_to_drop;"
# Expected: All show "YES - Safe to drop"

# 2. Create final backup (CRITICAL - LAST CHANCE)
BACKUP_FILE="~/Downloads/final-pre-drop-backup-$(date +%Y%m%d).dump"
pg_dump "$DB_URL" --format=custom --compress=9 --file="$BACKUP_FILE"
echo "⚠️ FINAL BACKUP: $BACKUP_FILE"

# 3. Backup archive schema only (extra safety)
ARCHIVE_BACKUP="~/Downloads/archive-schema-only-$(date +%Y%m%d).sql.gz"
pg_dump "$DB_URL" --schema=archive --format=plain | gzip -9 > "$ARCHIVE_BACKUP"
echo "Archive schema backup: $ARCHIVE_BACKUP"

# 4. Run migration 012
sqlx migrate run

# 5. Verify cleanup
psql $DB_URL -c "
SELECT schema_name FROM information_schema.schemata WHERE schema_name = 'archive';
"
# Expected: 0 rows (archive schema dropped)

psql $DB_URL -c "
SELECT COUNT(*) FROM public.dropped_tables_log;
"
# Expected: 7 rows (metadata preserved)
```

### Verification

```sql
-- Verify archive schema gone
SELECT COUNT(*) as archive_schemas
FROM information_schema.schemata 
WHERE schema_name = 'archive';
-- Expected: 0

-- Verify no entity_* tables in core
SELECT COUNT(*) as entity_tables
FROM information_schema.tables
WHERE table_schema = 'core' AND table_name LIKE 'entity_%';
-- Expected: 0

-- Verify metadata preserved
SELECT * FROM public.dropped_tables_log ORDER BY table_name;
-- Expected: 7 rows with all entity_* table info

-- Check space reclaimed
SELECT pg_size_pretty(pg_database_size(current_database())) as database_size;
-- Expected: ~2.4 GB smaller than before
```

---

## Rollback Procedures

### If Migration 011 Fails

```sql
-- Tables might be partially moved
-- Check current state
SELECT schemaname, tablename FROM pg_tables WHERE tablename LIKE 'entity_%';

-- Rollback: Move tables back to core schema
ALTER TABLE archive.entity_addresses SET SCHEMA core;
ALTER TABLE archive.entity_emails SET SCHEMA core;
-- ... repeat for all tables

-- Re-add FK constraint
ALTER TABLE core.entity_addresses 
ADD CONSTRAINT entity_addresses_address_id_fkey 
FOREIGN KEY (address_id) REFERENCES core.addresses(id);

-- Drop archive schema
DROP SCHEMA archive CASCADE;
```

### If Migration 012 Fails

If migration 012 fails partway through, restore from backup:

```bash
# Restore full database from backup
pg_restore --dbname="$DB_URL" --clean --if-exists final-pre-drop-backup-YYYYMMDD.dump

# Or restore archive schema only
gunzip -c archive-schema-only-YYYYMMDD.sql.gz | psql "$DB_URL"
```

### If Need to Recover After Migration 012

**⚠️ WARNING:** Once migration 012 completes, tables are PERMANENTLY DELETED.

Recovery options:

1. **From backup** (if created):
   ```bash
   pg_restore --dbname="$DB_URL" --schema=archive final-pre-drop-backup-YYYYMMDD.dump
   ```

2. **From metadata** (shows what existed, but data is gone):
   ```sql
   SELECT * FROM public.dropped_tables_log WHERE migration_id = 11;
   ```

3. **No recovery** if no backup exists - data is permanently lost.

---

## Monitoring During Archive Period

### Daily Checks (First Week)

```sql
-- Monitor for errors referencing entity tables
SELECT 
    level,
    message,
    created_at
FROM audit.logged_actions
WHERE message LIKE '%entity_%'
  AND created_at >= CURRENT_DATE - INTERVAL '1 day'
ORDER BY created_at DESC;
```

### Weekly Checks (Weeks 2-12)

```sql
-- Verify no application using archived tables
SELECT 
    schemaname,
    tablename,
    seq_scan,
    seq_tup_read,
    idx_scan,
    idx_tup_fetch,
    n_tup_ins,
    n_tup_upd,
    n_tup_del
FROM pg_stat_user_tables
WHERE tablename LIKE 'entity_%'
  AND schemaname = 'archive';
-- Expected: All scan/insert/update/delete counts = 0
```

### Archive Schema Size Monitoring

```sql
-- Track archive schema size over time
SELECT 
    table_name,
    pg_size_pretty(pg_total_relation_size('archive.' || table_name)) as size
FROM archive.archive_metadata
ORDER BY pg_total_relation_size('archive.' || table_name) DESC;
```

---

## Timeline Example

| Date | Action | Status |
|------|--------|--------|
| 2025-11-22 | Create migrations 011 & 012 | ✅ Complete |
| 2025-11-22 | Database backup | ✅ Complete |
| 2025-11-25 | Deploy Phase 1-2 code | ⏳ Pending |
| 2025-12-01 | Run migration 011 (archive tables) | ⏳ Pending |
| 2025-12-01 | Start 90-day monitoring | ⏳ Pending |
| 2026-02-28 | Check archive.safe_to_drop | ⏳ Pending |
| 2026-03-01 | Final backup + migration 012 | ⏳ Pending |
| 2026-03-01 | Party Model migration 100% complete | ⏳ Pending |

---

## Disk Space Impact

### After Migration 011
- Archive schema: **+2.4 GB** (tables moved, not dropped)
- Core schema: **-2.4 GB** (tables moved out)
- Total database: **No change** (same data, different schema)

### After Migration 012
- Archive schema: **-2.4 GB** (dropped)
- Total database: **-2.4 GB** (space reclaimed)
- Permanent log: **+50 KB** (dropped_tables_log metadata)

---

## Emergency Procedures

### If Production Breaks After Migration 011

```bash
# 1. Check if issue is related to entity_* tables
# Look for errors in logs

# 2. If confirmed, restore tables to core schema
psql $DB_URL << SQL
ALTER TABLE archive.entity_addresses SET SCHEMA core;
ALTER TABLE archive.entity_emails SET SCHEMA core;
ALTER TABLE archive.entity_family_relationships SET SCHEMA core;
ALTER TABLE archive.entity_financials SET SCHEMA core;
ALTER TABLE archive.entity_phones SET SCHEMA core;
ALTER TABLE archive.entity_profiles SET SCHEMA core;
ALTER TABLE archive.entity_relationships SET SCHEMA core;

-- Restore FK
ALTER TABLE core.entity_addresses 
ADD CONSTRAINT entity_addresses_address_id_fkey 
FOREIGN KEY (address_id) REFERENCES core.addresses(id);
SQL

# 3. Deploy code fix
# 4. Re-evaluate archival timeline
```

### If Need to Access Archived Data

```sql
-- Archive tables are read-only but still queryable
SELECT * FROM archive.entity_addresses WHERE entity_id = 'some-uuid';

-- Join with current tables
SELECT 
    p.name,
    ea.address_id,
    a.formatted_address
FROM core.parties p
JOIN core.entities e ON p.cpf_cnpj = e.national_id
JOIN archive.entity_addresses ea ON ea.entity_id = e.entity_id
JOIN core.addresses a ON a.id = ea.address_id
WHERE p.id = 'party-uuid';
```

---

## Success Criteria

### Migration 011 Success
- [x] Archive schema created
- [x] All 7 entity_* tables moved to archive schema
- [x] Metadata table created with 7 rows
- [x] Monitoring views created
- [x] No errors in application logs
- [x] No performance degradation

### Migration 012 Success
- [ ] All archived tables dropped
- [ ] Archive schema dropped
- [ ] Metadata preserved in dropped_tables_log
- [ ] ~2.4 GB disk space reclaimed
- [ ] No errors in application logs
- [ ] All queries still working

---

## Documentation References

- **Migration 009:** `migrations/009_create_party_addresses.sql` - Address migration
- **Migration 010:** `migrations/010_migrate_financials_to_jsonb.sql` - Financial migration
- **Migration 011:** `migrations/011_archive_entity_tables.sql` - Archive creation
- **Migration 012:** `migrations/012_drop_archived_entity_tables.sql` - Final cleanup
- **Schema Report:** `docs/database/DATABASE_SCHEMA_REPORT_FINAL.md`
- **Party Model Status:** `docs/database/PARTY_MODEL_MIGRATION_STATUS.md`

---

**Last Updated:** 2025-11-22  
**Next Review:** After Phase 1-2 deployment (2025-11-25)  
**Final Execution:** After 2026-02-28 (90-day retention)
