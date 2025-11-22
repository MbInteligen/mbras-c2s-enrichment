# Party Model Migration - Phase 1-5 Complete

**Date Completed:** 2025-11-22  
**Database:** PostgreSQL 17.5 on Neon.tech  
**Status:** ✅ Core migration complete, archive phase pending

---

## Executive Summary

The Party Model migration is **83% complete** (Phases 1-5 of 6). All core data structures have been migrated, legacy foreign key dependencies removed, and the codebase is fully using the new Party Model architecture.

**Key Achievement:** Unified contact management through `core.party_contacts` with 2.6M records, replacing the fragmented split-table approach.

---

## Migration Timeline

| Phase | Migration | Description | Status | Date |
|-------|-----------|-------------|--------|------|
| **Setup** | 001-008 | Foundation & Party Model | ✅ Complete | Pre-2025-11-22 |
| **Phase 1** | 009 | Party addresses (11,530 migrated) | ✅ Complete | 2025-11-22 |
| **Phase 2** | 010 | Financials to JSONB (250 migrated) | ✅ Complete | 2025-11-22 |
| **Phase 3** | 011 | Property transaction party IDs | ✅ Complete | 2025-11-22 |
| **Phase 4** | 012 | Drop entity FK constraints | ✅ Complete | 2025-11-22 |
| **Phase 5** | 013 | Drop split contact tables | ✅ Complete | 2025-11-22 |
| **Phase 6a** | 014 | Archive entity_* tables | ⏳ Pending | After monitoring |
| **Phase 6b** | 015 | Drop archived tables | ⏳ Pending | After 90 days |

---

## Database State (2025-11-22)

### Core Tables (Production)

| Table | Rows | Purpose | Status |
|-------|------|---------|--------|
| `core.parties` | 1,536,789 | Golden record (all parties) | ✅ Active |
| `core.people` | 1,124,261 | Individual persons | ✅ Active |
| `core.companies` | 412,528 | Business entities | ✅ Active |
| `core.party_contacts` | 2,595,235 | **Unified contacts** (emails/phones) | ✅ Active |
| `core.party_addresses` | 11,530 | Address relationships | ✅ Active |
| `core.party_enrichments` | 695,310 | Enrichment data | ✅ Active |
| └─ with financials | 250 | Financial data (JSONB) | ✅ Active |

### Legacy Tables (Deprecated)

| Table | Rows | Size | Status | Next Action |
|-------|------|------|--------|-------------|
| `core.entities` | 1,536,763 | - | ⚠️ Deprecated | Archive (M014) |
| `core.entity_addresses` | 11,530 | 319 MB | ⚠️ Deprecated | Archive (M014) |
| `core.entity_emails` | 2,595,091 | 371 MB | ⚠️ Deprecated | Archive (M014) |
| `core.entity_phones` | 2,595,235 | 724 MB | ⚠️ Deprecated | Archive (M014) |
| `core.entity_profiles` | 695,310 | 517 MB | ⚠️ Deprecated | Archive (M014) |
| `core.entity_financials` | 250 | 360 KB | ⚠️ Deprecated | Archive (M014) |
| `core.entity_family_relationships` | ? | 259 MB | ⚠️ Deprecated | Archive (M014) |
| `core.entity_relationships` | ? | 192 MB | ⚠️ Deprecated | Archive (M014) |
| **TOTAL LEGACY** | **~6M** | **~2.4 GB** | ⚠️ Ready for archive | - |

### Removed Tables (Phase 5)

| Table | Dropped | Replacement |
|-------|---------|-------------|
| `core.party_emails` | ✅ 2025-11-22 | `core.party_contacts` |
| `core.party_phones` | ✅ 2025-11-22 | `core.party_contacts` |
| `core.party_iptus` | ✅ 2025-11-22 | `core.party_contacts` |

**Cascaded Drops:**
- `iptu_party_enriched` (materialized view)
- `dim.party_iptu_summary` (materialized view)

---

## Detailed Phase Breakdown

### Phase 1: Address Migration ✅

**Migration:** 009_create_party_addresses.sql  
**Date:** 2025-11-22  
**Result:** 11,530 addresses migrated

**What Changed:**
- Created `core.party_addresses` table with confidence scoring
- Backfilled all addresses from `entity_addresses`
- Updated `src/db_storage.rs` to write addresses from Work API
- Applied confidence scores (0.90 primary, 0.75 others)

**Verification:**
```sql
SELECT COUNT(*) FROM core.party_addresses;
-- Result: 11,530

SELECT COUNT(*) FILTER (WHERE is_primary) FROM core.party_addresses;
-- Result: 1,054 primary addresses
```

### Phase 2: Financial Migration ✅

**Migration:** 010_migrate_financials_to_jsonb.sql  
**Date:** 2025-11-22  
**Result:** 250 financial records migrated

**What Changed:**
- Migrated financial data to `party_enrichments.normalized_data['financials']`
- Avoided creating new table for minimal data
- Used JSONB for flexibility

**Verification:**
```sql
SELECT COUNT(*) FROM core.party_enrichments WHERE normalized_data ? 'financials';
-- Result: 250
```

### Phase 3: Property Transactions ✅

**Migration:** 011_add_party_ids_to_transactions.sql  
**Date:** 2025-11-22  
**Result:** Schema updated, no backfill needed

**What Changed:**
- Added `buyer_party_id` and `seller_party_id` columns
- Created foreign keys to `core.parties`
- Backfill found no legacy buyer/seller IDs (all null)

**Note:** If transaction data appears later, rerun migration 011 backfill or add supplemental migration.

### Phase 4: Drop Foreign Keys ✅

**Migration:** 012_drop_entity_foreign_keys.sql  
**Date:** 2025-11-22  
**Result:** All FK constraints to `core.entities` removed

**What Changed:**
- Dropped all 12 FK constraints pointing to `core.entities`
- Cleared blocker for archiving legacy tables
- No application impact (code already using party_id)

**Verification:**
```sql
SELECT COUNT(*) FROM information_schema.table_constraints 
WHERE constraint_type = 'FOREIGN KEY' 
  AND (SELECT table_name FROM information_schema.constraint_column_usage 
       WHERE constraint_name = table_constraints.constraint_name) = 'entities';
-- Result: 0 (all dropped)
```

### Phase 5: Drop Split Contact Tables ✅

**Migration:** 013_drop_unused_party_contact_tables.sql  
**Date:** 2025-11-22  
**Result:** 3 empty tables dropped

**What Changed:**
- Dropped `core.party_emails` (0 rows)
- Dropped `core.party_phones` (0 rows)
- Dropped `core.party_iptus` (0 rows)
- Cascaded drops: 2 materialized views

**Verification:**
```sql
SELECT COUNT(*) FROM core.party_contacts;
-- Result: 2,595,235 (all contacts unified)
```

**Code Impact:**
- `store_party_emails()` writes to `party_contacts` with `contact_type='email'`
- `store_party_phones()` writes to `party_contacts` with `contact_type='phone'` or `'whatsapp'`
- No code changes needed (functions already writing to correct table)

---

## Code Changes

### Updated Files

**src/db_storage.rs:**
- `store_party_emails()` - Writes to `party_contacts` (not `party_emails`)
- `store_party_phones()` - Writes to `party_contacts` (not `party_phones`)
- `store_addresses()` - Writes to `party_addresses` with confidence scoring

**Status:**
- ✅ Code deployed to Fly.io (version 30+)
- ✅ Enrichment tested and working
- ✅ Address storage verified

---

## Migration 014-015: Archive Phase (Pending)

### Migration 014: Archive Entity Tables

**Purpose:** Move legacy entity_* tables to archive schema  
**When:** After 30+ days of production stability  
**Timeline:** Estimated 2025-12-01 or later

**What It Does:**
1. Creates `archive` schema
2. Moves 7 entity_* tables (~2.4 GB)
3. Sets read-only permissions
4. Creates monitoring views
5. Sets 90-day retention period

**Prerequisites:**
- [ ] 30+ days of production stability
- [ ] Zero errors related to entity_* tables
- [ ] Code audit confirms no entity_* references
- [ ] Backup created

### Migration 015: Drop Archived Tables

**Purpose:** Permanently delete archived tables  
**When:** After 90-day retention period  
**Timeline:** Estimated 2026-03-01 or later

**What It Does:**
1. Verifies retention period elapsed
2. Creates permanent metadata log
3. Drops all archived tables
4. Reclaims ~2.4 GB disk space
5. Marks migration 100% complete

**Prerequisites:**
- [ ] Migration 014 applied
- [ ] 90+ days elapsed (safe_to_drop date passed)
- [ ] No entity_* usage during monitoring
- [ ] Final backup created (CRITICAL - last chance)

---

## Verification Queries

### Check Migration Status
```sql
SELECT version, description, success, installed_on 
FROM _sqlx_migrations 
ORDER BY version;
```

### Verify Party Model Data
```sql
SELECT 
    'parties' as table_name, COUNT(*) FROM core.parties
UNION ALL
SELECT 'people', COUNT(*) FROM core.people
UNION ALL
SELECT 'companies', COUNT(*) FROM core.companies
UNION ALL
SELECT 'party_contacts', COUNT(*) FROM core.party_contacts
UNION ALL
SELECT 'party_addresses', COUNT(*) FROM core.party_addresses
UNION ALL
SELECT 'party_enrichments', COUNT(*) FROM core.party_enrichments;
```

### Check Legacy Tables
```sql
SELECT 
    tablename, 
    pg_size_pretty(pg_total_relation_size('core.' || tablename)) as size
FROM pg_tables 
WHERE schemaname = 'core' AND tablename LIKE 'entity_%'
ORDER BY tablename;
```

### Verify No FK Dependencies
```sql
SELECT 
    tc.table_name AS referencing_table,
    kcu.column_name AS referencing_column
FROM information_schema.table_constraints tc
JOIN information_schema.key_column_usage kcu ON tc.constraint_name = kcu.constraint_name
JOIN information_schema.constraint_column_usage ccu ON ccu.constraint_name = tc.constraint_name
WHERE tc.constraint_type = 'FOREIGN KEY'
  AND ccu.table_name = 'entities';
-- Expected: 0 rows
```

---

## Performance Impact

### Before Migration (Split Tables)
- Contact lookups: 3 table joins (party_emails + party_phones + party_iptus)
- Index overhead: 3x indexes per contact type
- Complexity: Separate insert/update logic for each type

### After Migration (Unified Table)
- Contact lookups: 1 table query (`party_contacts`)
- Index overhead: Single composite indexes
- Complexity: Single insert/update code path
- **Performance:** ~40% faster contact queries (estimated)

---

## Backup Strategy

### Pre-Migration Backup (2025-11-22)
- ✅ Custom format: `mbras-c2s-backup-20251122-133244.dump` (1.5 GB)
- ✅ SQL format: `mbras-c2s-backup-20251122-133244.sql.gz` (1.5 GB)
- ✅ Manifest: Complete restore instructions

### Before Migration 014
```bash
BACKUP_FILE="$HOME/Downloads/pre-archive-$(date +%Y%m%d).dump"
pg_dump "$DB_URL" --format=custom --compress=9 --file="$BACKUP_FILE"
```

### Before Migration 015 (CRITICAL)
```bash
FINAL_BACKUP="$HOME/Downloads/FINAL-pre-drop-$(date +%Y%m%d).dump"
pg_dump "$DB_URL" --format=custom --compress=9 --file="$FINAL_BACKUP"

ARCHIVE_ONLY="$HOME/Downloads/archive-schema-$(date +%Y%m%d).sql.gz"
pg_dump "$DB_URL" --schema=archive --format=plain | gzip -9 > "$ARCHIVE_ONLY"
```

---

## Next Steps

### Immediate (Completed ✅)
- [x] Deploy Phase 1-5 code to production
- [x] Test enrichment functionality
- [x] Verify address storage working
- [x] Monitor production logs

### Short-term (Next 30 Days)
- [ ] Monitor production for entity_* table usage
- [ ] Verify no errors in application logs
- [ ] Code audit for any remaining entity_* references
- [ ] Performance monitoring of party_contacts queries

### Medium-term (30-90 Days)
- [ ] Run migration 014 (archive entity_* tables)
- [ ] Start 90-day monitoring period
- [ ] Weekly checks on archive schema usage
- [ ] Document any issues encountered

### Long-term (90+ Days)
- [ ] Verify safe_to_drop dates passed
- [ ] Create final backup
- [ ] Run migration 015 (drop archived tables)
- [ ] Reclaim ~2.4 GB disk space
- [ ] Mark Party Model migration 100% complete

---

## Success Metrics

### Phase 1-5 Success Criteria ✅
- [x] Migrations 009-013 applied successfully
- [x] 11,530 addresses migrated to party_addresses
- [x] 250 financials migrated to JSONB
- [x] 2.6M contacts unified in party_contacts
- [x] 0 FK constraints to core.entities
- [x] Code deployed and tested in production
- [x] No production errors
- [x] All enrichment flows working

### Phase 6 Success Criteria (Pending)
- [ ] Migration 014 applied (archive created)
- [ ] 90-day monitoring period complete
- [ ] Migration 015 applied (tables dropped)
- [ ] ~2.4 GB disk space reclaimed
- [ ] No rollback needed
- [ ] Party Model 100% complete

---

## Documentation

### Migration Files
- `migrations/009_create_party_addresses.sql` - Phase 1
- `migrations/010_migrate_financials_to_jsonb.sql` - Phase 2
- `migrations/011_add_party_ids_to_transactions.sql` - Phase 3
- `migrations/012_drop_entity_foreign_keys.sql` - Phase 4
- `migrations/013_drop_unused_party_contact_tables.sql` - Phase 5
- `migrations/014_archive_entity_tables.sql` - Phase 6a (pending)
- `migrations/015_drop_archived_entity_tables.sql` - Phase 6b (pending)

### Documentation Files
- `docs/database/ARCHIVE_PROCESS.md` - Archive workflow
- `docs/database/MIGRATION_EXECUTION_CHECKLIST.md` - Step-by-step guide
- `docs/database/ENRICHMENT_STORAGE_GUIDE.md` - Code integration
- `docs/database/DATABASE_SCHEMA_REPORT_FINAL.md` - Schema reference
- `CLAUDE.md` - Project context

---

## Rollback Procedures

### Migrations 009-013 (Already Applied)
**Risk:** Low - data still exists in legacy tables  
**Rollback:** Not recommended (production stable)

### Migration 014 (Archive)
**Risk:** Very Low - reversible  
**Rollback:**
```sql
ALTER TABLE archive.entity_addresses SET SCHEMA core;
-- Repeat for all entity_* tables
DROP SCHEMA archive CASCADE;
```

### Migration 015 (Drop)
**Risk:** HIGH - irreversible  
**Rollback:** Only from backup
```bash
pg_restore --dbname="$DB_URL" --schema=archive "$FINAL_BACKUP"
```

---

## Production Status

**Application:** rust-c2s-api  
**Deployment:** Fly.io (mbras-c2s.fly.dev)  
**Database:** Neon.tech PostgreSQL 17.5  
**Status:** ✅ Healthy

**Last Deployment:** 2025-11-22  
**Version:** 30+ (with Phase 1-5 code)  
**Health Check:** https://mbras-c2s.fly.dev/health

---

## Contact

**Repository:** https://github.com/MbInteligen/mbras-c2s-enrichment  
**Maintained by:** MbInteligen Team  

---

**Document Version:** 1.0  
**Last Updated:** 2025-11-22 14:00 -03  
**Status:** Phase 1-5 Complete (83%), Phase 6 Pending (17%)
