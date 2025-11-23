# Migration Execution Checklist

**Purpose:** Step-by-step checklist for safely executing Party Model migrations  
**Last Updated:** 2025-11-22

---

## Migration 011: Archive Entity Tables

**⚠️ DO NOT RUN UNTIL:**
- [ ] Migrations 009-010 applied to production ✅ (DONE 2025-11-22)
- [ ] Phase 1-2 code deployed to Fly.io
- [ ] Address storage tested and working in production
- [ ] At least 30 days of stable production usage
- [ ] Zero errors related to entity_* tables in logs
- [ ] Code audit completed (no entity_* references found)

### Pre-Migration Checklist

```bash
# 1. Verify current migration state
psql $DB_URL -c "SELECT version, description FROM _sqlx_migrations ORDER BY version;"
# Expected: Migrations 1-10 present

# 2. Check entity_* tables exist in core schema
psql $DB_URL -c "
SELECT tablename, pg_size_pretty(pg_total_relation_size('core.'||tablename))
FROM pg_tables 
WHERE schemaname = 'core' AND tablename LIKE 'entity_%' 
ORDER BY tablename;
"
# Expected: 7 tables (addresses, emails, family_relationships, financials, phones, profiles, relationships)

# 3. Verify no archive schema exists yet
psql $DB_URL -c "SELECT schema_name FROM information_schema.schemata WHERE schema_name = 'archive';"
# Expected: 0 rows

# 4. Check production logs for entity_* usage
# (Application-specific - check your logging system)

# 5. Create backup (CRITICAL)
BACKUP_FILE="$HOME/Downloads/pre-archive-$(date +%Y%m%d-%H%M%S).dump"
pg_dump "$DB_URL" --format=custom --compress=9 --file="$BACKUP_FILE"
ls -lh "$BACKUP_FILE"
echo "✅ Backup created: $BACKUP_FILE"
```

### Execute Migration

```bash
# Run migration
sqlx migrate run

# Or manually if sqlx not available
psql $DB_URL -f migrations/011_archive_entity_tables.sql
```

### Post-Migration Verification

```bash
# 1. Check archive schema created
psql $DB_URL -c "SELECT schema_name FROM information_schema.schemata WHERE schema_name = 'archive';"
# Expected: 1 row

# 2. Verify all tables moved to archive
psql $DB_URL -c "
SELECT schemaname, tablename, pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename))
FROM pg_tables 
WHERE tablename LIKE 'entity_%' 
ORDER BY schemaname, tablename;
"
# Expected: All 7 tables in 'archive' schema, 0 in 'core' schema

# 3. Check metadata created
psql $DB_URL -c "SELECT COUNT(*) FROM archive.archive_metadata;"
# Expected: 7 rows

# 4. Check safe_to_drop dates
psql $DB_URL -c "SELECT * FROM archive.safe_to_drop;"
# Expected: All show "NO - Wait until YYYY-MM-DD"

# 5. Monitor application logs
# Watch for any errors mentioning entity_* or relation not found
```

### Success Criteria
- ✅ Archive schema exists
- ✅ 7 entity_* tables in archive schema
- ✅ 0 entity_* tables in core schema
- ✅ Metadata table has 7 rows
- ✅ Safe_to_drop view shows 90-day retention dates
- ✅ No application errors in logs
- ✅ All API endpoints still working

---

## Migration 012: Drop Archived Tables

**⚠️ DO NOT RUN UNTIL:**
- [ ] Migration 011 applied (tables in archive schema)
- [ ] Current date >= 2026-02-28 (90+ days after archive)
- [ ] Production stable for entire 90-day period
- [ ] Zero entity_* usage detected during monitoring
- [ ] Final code audit confirms no entity_* references
- [ ] Business approval to permanently delete data

### Pre-Migration Checklist

```bash
# 1. Verify 90-day retention elapsed
psql $DB_URL -c "
SELECT 
    table_name,
    safe_to_drop_after,
    safe_to_drop_after - CURRENT_DATE as days_remaining,
    CASE 
        WHEN safe_to_drop_after <= CURRENT_DATE THEN '✅ SAFE'
        ELSE '❌ WAIT ' || (safe_to_drop_after - CURRENT_DATE) || ' days'
    END as status
FROM archive.archive_metadata
ORDER BY safe_to_drop_after;
"
# Expected: ALL show "✅ SAFE"

# 2. Verify archive schema exists
psql $DB_URL -c "SELECT schema_name FROM information_schema.schemata WHERE schema_name = 'archive';"
# Expected: 1 row

# 3. Check archive table usage during retention period
psql $DB_URL -c "
SELECT 
    schemaname,
    tablename,
    seq_scan as full_scans,
    idx_scan as index_scans,
    n_tup_ins as inserts,
    n_tup_upd as updates,
    n_tup_del as deletes
FROM pg_stat_user_tables
WHERE tablename LIKE 'entity_%' AND schemaname = 'archive';
"
# Expected: All scans/inserts/updates/deletes should be 0 or minimal (only monitoring queries)

# 4. Create FINAL backup (CRITICAL - LAST CHANCE)
FINAL_BACKUP="$HOME/Downloads/FINAL-pre-drop-$(date +%Y%m%d-%H%M%S).dump"
pg_dump "$DB_URL" --format=custom --compress=9 --file="$FINAL_BACKUP"
ls -lh "$FINAL_BACKUP"
echo "⚠️ FINAL BACKUP: $FINAL_BACKUP"

# 5. Backup archive schema separately (extra safety)
ARCHIVE_ONLY="$HOME/Downloads/archive-schema-only-$(date +%Y%m%d-%H%M%S).sql.gz"
pg_dump "$DB_URL" --schema=archive --format=plain | gzip -9 > "$ARCHIVE_ONLY"
ls -lh "$ARCHIVE_ONLY"
echo "✅ Archive schema backup: $ARCHIVE_ONLY"

# 6. Document backup locations
cat > "$HOME/Downloads/BACKUP_MANIFEST_$(date +%Y%m%d).txt" << EOF
Migration 012 Backups
Created: $(date)

FULL DATABASE BACKUP:
$FINAL_BACKUP
$(ls -lh "$FINAL_BACKUP")

ARCHIVE SCHEMA ONLY:
$ARCHIVE_ONLY
$(ls -lh "$ARCHIVE_ONLY")

To restore full database:
pg_restore --dbname="\$DB_URL" --clean --if-exists "$FINAL_BACKUP"

To restore archive schema only:
gunzip -c "$ARCHIVE_ONLY" | psql "\$DB_URL"

⚠️ Keep these backups for at least 1 year.
EOF
cat "$HOME/Downloads/BACKUP_MANIFEST_$(date +%Y%m%d).txt"
```

### Execute Migration

```bash
# ⚠️ POINT OF NO RETURN - Data will be permanently deleted

# Run migration
sqlx migrate run

# Or manually if sqlx not available
psql $DB_URL -f migrations/012_drop_archived_entity_tables.sql
```

### Post-Migration Verification

```bash
# 1. Verify archive schema deleted
psql $DB_URL -c "SELECT COUNT(*) FROM information_schema.schemata WHERE schema_name = 'archive';"
# Expected: 0

# 2. Verify no entity_* tables anywhere
psql $DB_URL -c "
SELECT schemaname, tablename 
FROM pg_tables 
WHERE tablename LIKE 'entity_%';
"
# Expected: 0 rows

# 3. Verify metadata preserved
psql $DB_URL -c "SELECT COUNT(*) FROM public.dropped_tables_log;"
# Expected: 7 rows

psql $DB_URL -c "SELECT table_name, dropped_at, replacement_table FROM public.dropped_tables_log ORDER BY table_name;"
# Expected: All 7 entity_* tables listed

# 4. Check database size reduction
psql $DB_URL -c "SELECT pg_size_pretty(pg_database_size(current_database()));"
# Expected: ~2.4 GB smaller than before

# 5. Monitor application logs
# Watch for any errors after migration

# 6. Test critical API endpoints
curl https://mbras-c2s.fly.dev/health
curl https://mbras-c2s.fly.dev/api/v1/c2s/enrich/TEST_LEAD_ID -X POST
# Expected: All working normally
```

### Success Criteria
- ✅ Archive schema dropped
- ✅ 0 entity_* tables in database
- ✅ Metadata preserved in dropped_tables_log (7 rows)
- ✅ ~2.4 GB disk space reclaimed
- ✅ No application errors
- ✅ All API endpoints working
- ✅ Party Model migration 100% complete

---

## Rollback Procedures

### Rollback Migration 011

If issues detected after archiving tables:

```bash
# Move tables back to core schema
psql $DB_URL << 'SQL'
ALTER TABLE archive.entity_addresses SET SCHEMA core;
ALTER TABLE archive.entity_emails SET SCHEMA core;
ALTER TABLE archive.entity_family_relationships SET SCHEMA core;
ALTER TABLE archive.entity_financials SET SCHEMA core;
ALTER TABLE archive.entity_phones SET SCHEMA core;
ALTER TABLE archive.entity_profiles SET SCHEMA core;
ALTER TABLE archive.entity_relationships SET SCHEMA core;

-- Restore FK constraint
ALTER TABLE core.entity_addresses 
ADD CONSTRAINT entity_addresses_address_id_fkey 
FOREIGN KEY (address_id) REFERENCES core.addresses(id);

-- Drop archive schema
DROP SCHEMA archive CASCADE;
SQL

echo "✅ Rollback complete - tables restored to core schema"
```

### Rollback Migration 012

⚠️ **Migration 012 cannot be rolled back** - data is permanently deleted.

Only option is restore from backup:

```bash
# Option 1: Restore full database
pg_restore --dbname="$DB_URL" --clean --if-exists "$FINAL_BACKUP"

# Option 2: Restore archive schema only
gunzip -c "$ARCHIVE_ONLY" | psql "$DB_URL"
```

**This is why backups before migration 012 are CRITICAL.**

---

## Monitoring During Archive Period

### Daily Monitoring (Week 1)

```sql
-- Check for errors mentioning entity tables
SELECT 
    level,
    message,
    created_at
FROM audit.logged_actions
WHERE message ILIKE '%entity%'
  AND created_at >= CURRENT_DATE - INTERVAL '1 day'
ORDER BY created_at DESC
LIMIT 100;
```

### Weekly Monitoring (Weeks 2-12)

```sql
-- Verify no accidental usage of archived tables
SELECT 
    schemaname,
    tablename,
    seq_scan,
    idx_scan,
    n_tup_ins,
    n_tup_upd,
    n_tup_del,
    last_vacuum,
    last_analyze
FROM pg_stat_user_tables
WHERE schemaname = 'archive'
ORDER BY tablename;
-- Expected: All modification counts = 0
```

### Monthly Review (Until Migration 012)

```sql
-- Check when safe to drop
SELECT * FROM archive.safe_to_drop;

-- Review archive size
SELECT * FROM archive.archive_size_report;

-- Verify party_* tables being used instead
SELECT 
    schemaname,
    tablename,
    n_tup_ins as inserts,
    n_tup_upd as updates
FROM pg_stat_user_tables
WHERE tablename IN ('party_addresses', 'party_emails', 'party_phones', 'party_enrichments')
  AND schemaname = 'core'
ORDER BY tablename;
-- Expected: Growing insert/update counts
```

---

## Emergency Contacts

If issues arise during migrations:

1. **Stop immediately** - don't continue if errors appear
2. **Check logs** - application and database logs
3. **Verify backups** - ensure backups are valid and restorable
4. **Rollback if needed** - use procedures above
5. **Investigate** - determine root cause before retrying
6. **Document** - record what went wrong for future reference

---

## Timeline Summary

| Date | Migration | Action | Rollback Risk |
|------|-----------|--------|---------------|
| 2025-11-22 | 009 | Create party_addresses | Low (additive) |
| 2025-11-22 | 010 | Migrate financials to JSONB | Low (additive) |
| 2025-11-25 | - | Deploy Phase 1-2 code | Medium (code change) |
| 2025-12-01 | 011 | Archive entity_* tables | Low (reversible) |
| 2025-12-01 | - | Start monitoring period | - |
| 2026-02-28 | - | Verify ready for cleanup | - |
| 2026-03-01 | 012 | DROP archived tables | **HIGH** (permanent) |

---

## Documentation

- **Archive Process:** `docs/database/ARCHIVE_PROCESS.md`
- **Migration 011:** `migrations/011_archive_entity_tables.sql`
- **Migration 012:** `migrations/012_drop_archived_entity_tables.sql`
- **Party Model Status:** `docs/database/PARTY_MODEL_MIGRATION_STATUS.md`

---

**Last Updated:** 2025-11-22  
**Next Action:** Deploy Phase 1-2 code, then schedule migration 011
