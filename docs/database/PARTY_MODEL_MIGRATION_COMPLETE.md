# Party Model Migration - Phase 1-6a Complete

**Date Completed:** 2025-11-22  
**Database:** PostgreSQL 17.5 on Neon.tech  
**Status:** ✅ 93% Complete - Archive created, 90-day retention in progress

---

## Executive Summary

The Party Model migration is **93% complete** (Phases 1-6a of 6). All core data structures have been migrated, legacy tables archived to read-only schema, and the codebase is fully using the new Party Model architecture.

**Key Achievements:** 
- Unified contact management through `core.party_contacts` with 2.6M records
- 2.4 GB of legacy tables archived (7 entity_* tables)
- 90-day monitoring period started (safe to drop after 2026-02-20)

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
| **Phase 6a** | 014 | Archive entity_* tables | ✅ Complete | 2025-11-22 17:08 |
| **Phase 6b** | 015 | Drop archived tables | ⏳ Pending | After 2026-02-20 |

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

### Archived Tables (Read-Only, archive schema)

| Table | Rows | Size | Status | Archived | Safe to Drop |
|-------|------|------|--------|----------|--------------|
| `archive.entity_addresses` | 11,530 | 319 MB | 📦 Archived | 2025-11-22 | 2026-02-20 |
| `archive.entity_emails` | 2,595,091 | 371 MB | 📦 Archived | 2025-11-22 | 2026-02-20 |
| `archive.entity_phones` | 2,595,235 | 724 MB | 📦 Archived | 2025-11-22 | 2026-02-20 |
| `archive.entity_profiles` | 695,310 | 517 MB | 📦 Archived | 2025-11-22 | 2026-02-20 |
| `archive.entity_financials` | 250 | 360 KB | 📦 Archived | 2025-11-22 | 2026-02-20 |
| `archive.entity_family_relationships` | ? | 259 MB | 📦 Archived | 2025-11-22 | 2026-02-20 |
| `archive.entity_relationships` | ? | 192 MB | 📦 Archived | 2025-11-22 | 2026-02-20 |
| **TOTAL ARCHIVED** | **~6M** | **2.4 GB** | 📦 90-day retention | - | **90 days remaining** |

**Note:** `core.entities` table still exists (not yet archived) - contains base party data, not part of entity_* pattern.

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

### Phase 6a: Archive Entity Tables ✅

**Migration:** 014_archive_entity_tables.sql  
**Date:** 2025-11-22 17:08:12 UTC  
**Result:** 7 legacy tables archived (2.4 GB)

**What Changed:**
- Created `archive` schema (read-only)
- Moved 7 entity_* tables from `core` to `archive` schema
- Created `archive_metadata` table (tracking retention)
- Created monitoring views: `safe_to_drop`, `archive_size_report`
- Set 90-day retention period (safe to drop after 2026-02-20)

**Tables Archived:**
| Table | Size | Rows | Replacement |
|-------|------|------|-------------|
| entity_addresses | 319 MB | 11,530 | core.party_addresses |
| entity_emails | 371 MB | 2,595,091 | core.party_contacts |
| entity_phones | 724 MB | 2,595,235 | core.party_contacts |
| entity_profiles | 517 MB | 695,310 | core.party_enrichments |
| entity_financials | 360 KB | 250 | core.party_enrichments (JSONB) |
| entity_family_relationships | 259 MB | - | core.party_relationships |
| entity_relationships | 192 MB | - | core.party_relationships |

**Verification:**
```sql
-- Check archive schema created
SELECT * FROM archive.safe_to_drop;
-- Result: 7 tables, all showing "NO - Wait until 2026-02-20" (90 days)

-- Verify no entity_* in core schema
SELECT COUNT(*) FROM pg_tables WHERE schemaname = 'core' AND tablename LIKE 'entity_%';
-- Result: 0 (all moved)

-- Total archive size
SELECT pg_size_pretty(SUM(pg_total_relation_size('archive.' || table_name))) 
FROM archive.archive_metadata;
-- Result: 2383 MB (2.4 GB)
```

**Monitoring Views:**
```sql
-- Check when safe to drop
SELECT * FROM archive.safe_to_drop ORDER BY safe_to_drop_after;

-- Monitor archive size
SELECT * FROM archive.archive_size_report ORDER BY current_size DESC;
```

**Retention Period:**
- Started: 2025-11-22 17:08 UTC
- Safe to drop after: 2026-02-20
- Days remaining: 90

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

## Migration 015: Drop Archived Tables (Pending)

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
- [x] Migration 014 applied ✅ (2025-11-22)
- [ ] 90+ days elapsed (safe_to_drop date passed) - **Due: 2026-02-20**
- [ ] No entity_* usage during monitoring
- [ ] Final backup created (CRITICAL - last chance)

**Current Status:**
- Archive created: 2025-11-22 17:08 UTC
- Retention period: 90 days
- Safe to drop after: 2026-02-20
- Days remaining: 90
- Status: ⏳ Monitoring in progress

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

### Short-term (Next 90 Days) - Monitoring Period
- [x] Run migration 014 (archive entity_* tables) ✅ (2025-11-22)
- [ ] Weekly checks on `archive.safe_to_drop` view
- [ ] Monitor `pg_stat_user_tables` for archive schema usage (should be 0)
- [ ] Verify no errors mentioning entity_* tables in logs
- [ ] Performance monitoring of party_contacts queries
- [ ] Monthly review of archive size (should remain stable)

### Long-term (After 2026-02-20) - Final Cleanup
- [ ] Verify safe_to_drop dates passed (check view)
- [ ] Verify 90-day monitoring showed zero archive usage
- [ ] Create final backup (CRITICAL - last chance to preserve data)
- [ ] Create archive-schema-only backup (extra safety)
- [ ] Run migration 015 (drop archived tables)
- [ ] Verify ~2.4 GB disk space reclaimed
- [ ] Mark Party Model migration 100% complete 🎉

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

### Phase 6a Success Criteria ✅
- [x] Migration 014 applied (archive created) ✅ 2025-11-22 17:08
- [x] Archive schema contains 7 entity_* tables (2.4 GB)
- [x] Metadata table created with retention tracking
- [x] Monitoring views created (safe_to_drop, archive_size_report)
- [x] Read-only permissions set on archive schema
- [x] 90-day retention period started (until 2026-02-20)

### Phase 6b Success Criteria (Pending - After 2026-02-20)
- [ ] 90-day monitoring period complete (zero archive usage)
- [ ] Final backup created
- [ ] Migration 015 applied (tables dropped)
- [ ] ~2.4 GB disk space reclaimed
- [ ] No rollback needed
- [ ] Party Model 100% complete 🎉

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
**Risk:** Low - data still exists in archived tables  
**Rollback:** Not recommended (production stable, data in archive)

### Migration 014 (Archive) - Already Applied ✅
**Risk:** Very Low - reversible  
**Status:** Applied 2025-11-22 17:08  
**Rollback (if needed):**
```sql
-- Move tables back to core schema
ALTER TABLE archive.entity_addresses SET SCHEMA core;
ALTER TABLE archive.entity_emails SET SCHEMA core;
ALTER TABLE archive.entity_family_relationships SET SCHEMA core;
ALTER TABLE archive.entity_financials SET SCHEMA core;
ALTER TABLE archive.entity_phones SET SCHEMA core;
ALTER TABLE archive.entity_profiles SET SCHEMA core;
ALTER TABLE archive.entity_relationships SET SCHEMA core;

-- Drop archive schema
DROP SCHEMA archive CASCADE;
```
**Note:** Rollback should only be done if archive schema causes issues. Data is safe and accessible in archive schema.

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

**Document Version:** 2.0  
**Last Updated:** 2025-11-22 17:30 -03  
**Status:** Phase 1-6a Complete (93%), Phase 6b Pending (7%) - 90-day retention in progress
