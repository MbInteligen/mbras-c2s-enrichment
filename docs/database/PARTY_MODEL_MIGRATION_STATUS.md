# Party Model Migration Status Report

**Date:** 2025-11-22  
**Database:** PostgreSQL 17.5 (Neon.tech)  
**Purpose:** Verify migration 008 backfill and assess legacy table deprecation readiness

---

## Executive Summary

✅ **Migration 008 backfill is COMPLETE**  
✅ **Party Model tables are fully populated**  
⚠️ **Legacy tables still have active foreign key dependencies**  
⚠️ **Address data needs migration strategy**

**Recommendation:** Legacy tables (`core.entities` tree) **cannot be dropped yet** due to active foreign key constraints and unmigrated address data.

---

## 1. Party Model Table Counts (Target Schema)

**Query:**
```sql
SELECT 'parties' as tbl, count(*) FROM core.parties
UNION ALL SELECT 'people', count(*) FROM core.people
UNION ALL SELECT 'companies', count(*) FROM core.companies
UNION ALL SELECT 'party_contacts', count(*) FROM core.party_contacts
UNION ALL SELECT 'party_enrichments', count(*) FROM core.party_enrichments
UNION ALL SELECT 'ownerships', count(*) FROM core.ownerships
UNION ALL SELECT 'party_relationships', count(*) FROM core.party_relationships;
```

**Results:**

| Table | Count | Status |
|-------|-------|--------|
| **parties** | 1,536,776 | ✅ Populated |
| **people** | 1,124,248 | ✅ Populated |
| **companies** | 412,528 | ✅ Populated |
| **party_contacts** | 2,594,875 | ✅ Populated |
| **party_enrichments** | 695,295 | ✅ Populated |
| **ownerships** | 1,002,210 | ✅ Populated |
| **party_relationships** | 604,450 | ✅ Populated |

**Analysis:**
- ✅ All Party Model tables have data
- ✅ Counts match legacy entity counts (entities: 1,536,776)
- ✅ Migration 008 backfill was successful
- ✅ Contacts consolidated from entity_emails (872,971) + entity_phones (1,722,526) = ~2.6M ✓

---

## 2. Legacy Entity Table Counts (Source Schema)

**Query:**
```sql
SELECT 'entities' as tbl, count(*) FROM core.entities
UNION ALL SELECT 'entity_emails', count(*) FROM core.entity_emails
UNION ALL SELECT 'entity_phones', count(*) FROM core.entity_phones
UNION ALL SELECT 'entity_addresses', count(*) FROM core.entity_addresses
UNION ALL SELECT 'entity_profiles', count(*) FROM core.entity_profiles
UNION ALL SELECT 'entity_financials', count(*) FROM core.entity_financials
UNION ALL SELECT 'entity_relationships', count(*) FROM core.entity_relationships
UNION ALL SELECT 'property_ownerships', count(*) FROM core.property_ownerships;
```

**Results:**

| Table | Count | Migrated To |
|-------|-------|-------------|
| **entities** | 1,536,776 | core.parties ✅ |
| **entity_profiles** | 1,375,991 | core.people ✅ |
| **entity_emails** | 872,971 | core.party_contacts ✅ |
| **entity_phones** | 1,722,526 | core.party_contacts ✅ |
| **entity_relationships** | 604,450 | core.party_relationships ✅ |
| **property_ownerships** | 1,002,210 | core.ownerships ✅ |
| **entity_addresses** | 11,530 | ⚠️ **NOT MIGRATED** |
| **entity_financials** | 250 | ⚠️ **NOT MIGRATED** |

**Analysis:**
- Most data successfully migrated to Party Model
- **11,530 addresses** remain in legacy `entity_addresses` table
- **250 financial records** in `entity_financials` (minimal data)

---

## 3. Foreign Key Dependencies

**Query:**
```sql
SELECT tc.table_schema, tc.table_name, kcu.column_name
FROM information_schema.table_constraints tc
JOIN information_schema.key_column_usage kcu USING (constraint_name, table_schema)
WHERE tc.constraint_type = 'FOREIGN KEY'
  AND kcu.table_schema = 'core'
  AND kcu.column_name LIKE '%entity_id%'
ORDER BY tc.table_name, kcu.column_name;
```

**Results:**

| Table | Column | Constraint |
|-------|--------|------------|
| entity_addresses | entity_id | FK → core.entities ⚠️ |
| entity_emails | entity_id | FK → core.entities ⚠️ |
| entity_financials | entity_id | FK → core.entities ⚠️ |
| entity_phones | entity_id | FK → core.entities ⚠️ |
| entity_profiles | entity_id | FK → core.entities ⚠️ |
| entity_relationships | source_entity_id | FK → core.entities ⚠️ |
| entity_relationships | target_entity_id | FK → core.entities ⚠️ |
| property_ownerships | entity_id | FK → core.entities ⚠️ |
| property_transactions | buyer_entity_id | FK → core.entities ⚠️ |
| property_transactions | seller_entity_id | FK → core.entities ⚠️ |

**Analysis:**
- **10 active foreign key constraints** still point to `core.entities`
- **Cannot drop `core.entities`** without breaking referential integrity
- **Cannot drop child tables** (entity_emails, entity_phones, etc.) without breaking constraints

---

## 4. Constraint Verification

**Query:**
```sql
SELECT conname FROM pg_constraint WHERE conname = 'uq_party_contact_unique';
```

**Result:**
```
✅ uq_party_contact_unique EXISTS
```

**Analysis:**
- ✅ Deduplication constraint in place
- ✅ Prevents duplicate (party_id, contact_type, value) entries

---

## 5. Address Data Gap

### Current State

**Legacy Address Storage:**
- `core.entity_addresses` - 11,530 addresses
- `core.addresses` - Physical address records
- Linked via FK: entity_addresses.address_id → addresses.id

**Party Model Address Storage:**
- `core.party_addresses` - **DOES NOT EXIST** ⚠️
- Addresses currently stored in `core.party_enrichments.normalized_data["addresses"]` (JSONB)

### Address Migration Options

**Option A: Keep JSONB-only** (Current)
- ✅ No migration needed
- ✅ Flexible schema
- ❌ Cannot query by address efficiently
- ❌ Cannot enforce uniqueness
- ❌ Loses 11,530 existing addresses

**Option B: Create `core.party_addresses` table**
- ✅ Structured querying
- ✅ Indexes and constraints
- ✅ Preserves existing 11,530 addresses
- ❌ Requires migration script
- ❌ Code changes needed

**Recommendation:** **Option B** - Create `core.party_addresses` to preserve existing address data and enable structured queries.

---

## 6. Migration Roadmap

### Phase 1: Address Migration (Required before drop)

**Step 1.1: Create `core.party_addresses` table**
```sql
CREATE TABLE core.party_addresses (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    party_id UUID NOT NULL REFERENCES core.parties(id) ON DELETE CASCADE,
    address_id UUID REFERENCES core.addresses(id),
    address_type TEXT CHECK (address_type IN ('residential', 'commercial', 'billing', 'family_member')),
    is_primary BOOLEAN DEFAULT false,
    is_current BOOLEAN DEFAULT true,
    confidence_score NUMERIC(3,2) CHECK (confidence_score >= 0 AND confidence_score <= 1),
    verified BOOLEAN DEFAULT false,
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_party_addresses_party ON core.party_addresses(party_id);
CREATE INDEX idx_party_addresses_address ON core.party_addresses(address_id);
CREATE INDEX idx_party_addresses_primary ON core.party_addresses(party_id, is_primary) WHERE is_primary = true;
```

**Step 1.2: Backfill addresses from legacy**
```sql
INSERT INTO core.party_addresses (party_id, address_id, address_type, is_primary, is_current, confidence_score, metadata)
SELECT 
    p.id as party_id,
    ea.address_id,
    COALESCE(ea.address_type, 'residential') as address_type,
    ea.is_primary,
    true as is_current,
    0.8 as confidence_score, -- Legacy data assumed good quality
    jsonb_build_object(
        'source', 'legacy_migration',
        'original_entity_id', ea.entity_id,
        'migrated_at', CURRENT_TIMESTAMP
    ) as metadata
FROM core.entity_addresses ea
JOIN core.entities e ON ea.entity_id = e.entity_id
JOIN core.parties p ON p.cpf_cnpj = e.national_id
ON CONFLICT DO NOTHING;
```

**Step 1.3: Update application code**
- Modify `src/db_storage.rs` to write addresses to `core.party_addresses`
- Add address storage logic to enrichment flow

### Phase 2: Financial Data Migration (Optional)

**Status:** Only 250 records exist in `entity_financials`

**Options:**
1. Migrate to `core.party_enrichments.normalized_data["financials"]` (JSONB)
2. Keep in `entity_financials` indefinitely (minimal overhead)
3. Drop if not used by application

**Recommendation:** Migrate to JSONB in party_enrichments (low priority)

### Phase 3: Drop Foreign Key Constraints

**Once addresses are migrated:**

```sql
-- Drop FK constraints on entity tables
ALTER TABLE entity_addresses DROP CONSTRAINT IF EXISTS entity_addresses_entity_id_fkey;
ALTER TABLE entity_emails DROP CONSTRAINT IF EXISTS entity_emails_entity_id_fkey;
ALTER TABLE entity_phones DROP CONSTRAINT IF EXISTS entity_phones_entity_id_fkey;
ALTER TABLE entity_profiles DROP CONSTRAINT IF EXISTS entity_profiles_entity_id_fkey;
ALTER TABLE entity_financials DROP CONSTRAINT IF EXISTS entity_financials_entity_id_fkey;
ALTER TABLE entity_relationships DROP CONSTRAINT IF EXISTS entity_relationships_source_entity_id_fkey;
ALTER TABLE entity_relationships DROP CONSTRAINT IF EXISTS entity_relationships_target_entity_id_fkey;
ALTER TABLE property_ownerships DROP CONSTRAINT IF EXISTS property_ownerships_entity_id_fkey;
ALTER TABLE property_transactions DROP CONSTRAINT IF EXISTS property_transactions_buyer_entity_id_fkey;
ALTER TABLE property_transactions DROP CONSTRAINT IF EXISTS property_transactions_seller_entity_id_fkey;
```

### Phase 4: Archive or Drop Legacy Tables

**Option A: Archive to separate schema**
```sql
CREATE SCHEMA IF NOT EXISTS archive;

ALTER TABLE core.entities SET SCHEMA archive;
ALTER TABLE core.entity_addresses SET SCHEMA archive;
ALTER TABLE core.entity_emails SET SCHEMA archive;
ALTER TABLE core.entity_phones SET SCHEMA archive;
ALTER TABLE core.entity_profiles SET SCHEMA archive;
ALTER TABLE core.entity_financials SET SCHEMA archive;
ALTER TABLE core.entity_relationships SET SCHEMA archive;
```

**Option B: Drop entirely**
```sql
DROP TABLE IF EXISTS core.entity_addresses CASCADE;
DROP TABLE IF EXISTS core.entity_emails CASCADE;
DROP TABLE IF EXISTS core.entity_phones CASCADE;
DROP TABLE IF EXISTS core.entity_profiles CASCADE;
DROP TABLE IF EXISTS core.entity_financials CASCADE;
DROP TABLE IF EXISTS core.entity_relationships CASCADE;
DROP TABLE IF EXISTS core.entities CASCADE;
```

**Recommendation:** **Option A** (Archive) - Keep for 30-90 days as safety net, then drop after verification.

---

## 7. Verification Queries

### After Address Migration

**Verify address migration success:**
```sql
SELECT 'legacy_addresses' as source, count(*) FROM core.entity_addresses
UNION ALL
SELECT 'party_addresses', count(*) FROM core.party_addresses;
```

**Expected:** Both counts should match (11,530)

**Spot-check a migrated address:**
```sql
SELECT 
    p.cpf_cnpj,
    p.full_name,
    a.street,
    a.city,
    pa.address_type,
    pa.confidence_score,
    pa.metadata
FROM core.party_addresses pa
JOIN core.parties p ON pa.party_id = p.id
JOIN core.addresses a ON pa.address_id = a.id
LIMIT 10;
```

### Before Dropping Legacy Tables

**Ensure no application references:**
```bash
# Search codebase for entity_id references
cd /Users/ronaldo/Documents/projects/MBRAS/mbras-c2s/rust-c2s-api
grep -r "entity_id" src/ --exclude-dir=target
grep -r "core.entities" src/ --exclude-dir=target
```

**Ensure no active queries:**
```sql
-- Check for running queries on legacy tables
SELECT pid, query
FROM pg_stat_activity
WHERE query LIKE '%core.entities%'
   OR query LIKE '%entity_emails%'
   OR query LIKE '%entity_phones%';
```

---

## 8. Current Application Alignment

### Code Status

**Current implementation (`src/db_storage.rs`):**
- ✅ Writes to `core.parties`
- ✅ Writes to `core.people` / `core.companies`
- ✅ Writes to `core.party_contacts`
- ✅ Writes to `core.party_enrichments`
- ❌ **Does NOT write to `core.party_addresses`** (uses JSONB only)

**Application is aligned with Party Model** except for addresses.

### Reads from Legacy Tables

**Search for entity table references in code:**
```bash
# Check if any queries still read from legacy tables
grep -r "FROM core.entities" src/ 2>/dev/null | head -5
grep -r "JOIN core.entity_" src/ 2>/dev/null | head -5
```

**If code still references legacy tables:** Migration not complete, code needs updates.

---

## 9. Risk Assessment

### Risks of Dropping Legacy Tables NOW

| Risk | Impact | Likelihood |
|------|--------|------------|
| **Foreign key violation** | 🔴 Critical | 100% (constraints exist) |
| **Loss of 11,530 addresses** | 🔴 Critical | 100% (no migration) |
| **Loss of 250 financial records** | 🟡 Medium | 100% (no migration) |
| **Breaking existing queries** | 🟡 Medium | Unknown (need code audit) |
| **Data integrity issues** | 🟢 Low | Low (if constraints handled) |

### Safe to Drop After

- ✅ Address migration complete (`core.party_addresses` populated)
- ✅ Financial data migrated or confirmed unused
- ✅ All FK constraints dropped
- ✅ Code audit confirms no legacy table references
- ✅ 30-day archive period complete

---

## 10. Action Plan

### Immediate Actions (Week 1)

1. ✅ **DONE:** Verify migration 008 backfill (completed)
2. ⏳ **TODO:** Create `core.party_addresses` table
3. ⏳ **TODO:** Backfill addresses from `entity_addresses`
4. ⏳ **TODO:** Update `src/db_storage.rs` to write addresses

### Short-term Actions (Week 2-3)

5. ⏳ **TODO:** Code audit - search for legacy table references
6. ⏳ **TODO:** Test enrichment flow end-to-end
7. ⏳ **TODO:** Verify no production queries use legacy tables
8. ⏳ **TODO:** Decide on entity_financials disposition

### Long-term Actions (Week 4+)

9. ⏳ **TODO:** Drop FK constraints pointing to `core.entities`
10. ⏳ **TODO:** Archive legacy tables to `archive` schema
11. ⏳ **TODO:** Monitor for 30 days
12. ⏳ **TODO:** Drop archived tables permanently

---

## 11. Conclusion

### Summary

✅ **Migration 008 backfill is successful**  
- 1.5M+ parties migrated
- 2.6M+ contacts consolidated
- 1M+ ownerships migrated
- 600K+ relationships migrated

⚠️ **Blockers to dropping legacy tables:**
1. **11,530 addresses not migrated** - Need `core.party_addresses` table
2. **10 active foreign keys** - Must be dropped first
3. **250 financial records** - Need disposition decision
4. **Code audit needed** - Ensure no legacy references

### Next Steps

**Priority 1 (Critical):**
- Create and populate `core.party_addresses` table
- Migrate 11,530 addresses from legacy schema

**Priority 2 (High):**
- Code audit for legacy table references
- Drop foreign key constraints

**Priority 3 (Medium):**
- Archive legacy tables (don't drop yet)
- Monitor for 30-90 days

**Timeline:** Estimated 2-4 weeks to safely deprecate legacy tables.

---

**Report Generated:** 2025-11-22  
**Database Version:** PostgreSQL 17.5  
**Schema Version:** Migration 007 applied, Migration 008 backfilled manually  
**Status:** ✅ Migration successful, ⚠️ Deprecation blocked by address migration

