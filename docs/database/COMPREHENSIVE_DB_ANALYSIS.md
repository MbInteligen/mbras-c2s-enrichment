# Comprehensive Database Analysis - Independent Review

**Date:** 2025-11-22  
**Database:** neondb (PostgreSQL 17.5)  
**Analysis Type:** Fresh, migration-independent perspective  
**Total Database Size:** 16 GB

---

## Executive Summary

This database contains a **mature real estate and party (person/company) data platform** with:
- **1.5M+ parties** (people and companies)
- **1.4M+ properties** with transaction history
- **2.6M+ contact records** (emails, phones)
- **Extensive audit trail** (3.3 GB of logged actions)
- **2.4 GB archive** of legacy data structures

### Key Observations

✅ **Strengths:**
- No duplicate CPF/CNPJ identifiers (100% unique)
- No orphaned party records (all have type classification)
- Comprehensive indexing strategy (3.2 GB of indexes)
- Strong audit trail (4.6 GB audit schema)
- 45% enrichment rate (695K of 1.5M parties)

⚠️ **Opportunities:**
- Contact coverage: Only 27% of parties have contact info
- Materialized views using archived tables (potential stale data)
- Large audit tables (3.3 GB logged_actions)
- Zero property transactions linked to parties (migration in progress)

---

## Schema Architecture

### 8 Active Schemas

| Schema | Tables | Views | Mat Views | Size | Purpose |
|--------|--------|-------|-----------|------|---------|
| **core** | 15 | 3 | 4 | 8.2 GB | Primary business data |
| **audit** | 4 | 3 | 0 | 4.6 GB | Change tracking & compliance |
| **archive** | 9 | 2 | 0 | 3.3 GB | Deprecated legacy structures |
| **analytics** | 0 | 0 | 1 | 324 MB | Marketing analytics |
| **ref** | 9 | 0 | 0 | 17 MB | Reference/lookup data |
| **public** | 4 | 2 | 0 | 7.9 MB | System tables, webhooks |
| **app** | 3 | 0 | 0 | 72 KB | Application layer (empty) |
| **neon_auth** | 1 | 0 | 0 | 80 KB | Authentication |

---

## Core Schema Deep Dive (8.2 GB)

### Party Management (Primary Entity Model)

**core.parties** - Golden Record (725 MB, 1.5M rows)
- Type: Person (73%) vs Company (27%)
- Enrichment: 695K enriched (45.24%)
- Unique identifiers: 100% (no duplicates)
- Storage: 231 MB data + 494 MB indexes (68% index overhead)

**core.people** - Individuals (216 MB, 1.1M rows)
- All have corresponding party record (no orphans)
- Birth date coverage: Unknown (column exists)
- Storage: 126 MB data + 90 MB indexes

**core.companies** - Businesses (84 MB, 413K rows)
- All have corresponding party record (no orphans)
- Storage: 50 MB data + 34 MB indexes

### Contact Management (Unified Model)

**core.party_contacts** - Unified Contacts (1.2 GB, 2.6M rows)
- **Largest table by total size** (including indexes)
- Phone contacts: 1.7M (66%)
- Email contacts: 873K (34%)
- Primary contacts: 42K (1.6%)
- Verified contacts: 5.5K (0.2%)
- Coverage: Only 413K parties (27% of total)

**Contact Type Distribution:**
| Type | Count | Unique Parties | Primary | Verified | WhatsApp |
|------|-------|----------------|---------|----------|----------|
| Phone | 1.7M | 398K | 41K | 285 | 0 |
| Email | 873K | 232K | 1K | 5.2K | 0 |

**Storage Efficiency:**
- Table data: 322 MB
- Indexes: 911 MB (74% overhead - heavily indexed)
- Top indexes: pkey (347MB), value (273MB), unique constraint (181MB)

### Address Management

**core.addresses** - All Addresses (442 MB, 1.4M rows)
- Serves both parties and properties
- Storage: 308 MB data + 134 MB indexes

**core.party_addresses** - Party-Address Links (7.6 MB, 11.5K rows)
- Only 11K parties have addresses (0.75% coverage)
- Primary addresses: 1,054 (9.1%)
- Current addresses: 11,530 (100%)
- Verified: 1 (0.01%)

**Confidence Score Distribution:**
| Score | Count | Percentage | Meaning |
|-------|-------|------------|---------|
| 0.95 | 1 | 0.01% | Verified high confidence |
| 0.85 | 1,053 | 9.13% | Primary address |
| 0.75 | 10,476 | 90.86% | Secondary/additional |

**Average confidence:** 0.76

### Enrichment Data

**core.party_enrichments** - Enriched Data (349 MB, 695K rows)
- Enriched parties: 695K (45% of total)
- With financial data: 250 (0.04%)
- Storage: 168 MB data + 181 MB indexes

---

## Real Estate Data (3.5 GB)

### Properties

**core.real_estate_properties** - Properties (1.3 GB, 1.4M rows)
- Unique addresses: 1.3M (93%)
- With property type: 1.4M (100%)
- With area data: 1.4M (99%)
- With market value: 774K (53%)
- Average market value: R$ 6.78M

**Storage Breakdown:**
- Table data: 930 MB
- Indexes: 370 MB
- Zero live tuples reported (data quality issue?)

### Ownership & Transactions

**core.property_ownerships** - Ownerships (400 MB, records not counted)
- Storage: 286 MB data + 114 MB indexes

**core.property_transactions** - Transactions (317 MB, 1.1M rows)
- Date range: 1994-06-23 to 2025-08-06 (31 years)
- With transaction value: 1.1M (100%)
- **⚠️ Buyer party linkage: 0 (0%)**
- **⚠️ Seller party linkage: 0 (0%)**
- Buyer entity (legacy): 0
- Seller entity (legacy): 0

**Note:** Transaction-to-party linkage appears to be incomplete.

### Ownerships

**core.ownerships** - Legacy Ownership (318 MB, 1M rows)
- Links properties to parties
- Storage: 127 MB data + 191 MB indexes

---

## Materialized Views (2.9 GB)

### Performance Concern: Archive Dependencies

**core.mv_entity_enriched** - 1.3 GB
- **⚠️ DEPENDS ON ARCHIVE TABLES** (entity_emails, entity_phones, entity_addresses)
- Queries archive.entities instead of core.parties
- May contain stale data
- Should be rebuilt to use core.party_contacts, core.party_addresses

**core.mv_contributor_contacts** - 918 MB
- Purpose unknown (need definition analysis)
- Storage: 743 MB data + 176 MB indexes

**core.mv_transaction_enriched** - 375 MB
- Purpose unknown
- Storage: 265 MB data + 110 MB indexes

**analytics.mv_mkt_lead_star** - 324 MB
- Marketing lead analytics
- Storage: 194 MB data + 130 MB indexes

**core.mv_party_analytics** - 56 KB
- Minimal size, likely summary stats

**Recommendation:** Refresh or rebuild materialized views to use current core schema tables instead of archived data.

---

## Archive Schema (3.3 GB)

### Legacy Entity Model (Deprecated)

**archive.entities** - 951 MB, 1.5M rows
- Legacy golden record table
- Replaced by: core.parties

**Contact Tables:**
- **entity_phones** - 724 MB, 1.7M rows → Replaced by party_contacts
- **entity_emails** - 371 MB, 873K rows → Replaced by party_contacts

**Address Tables:**
- **entity_addresses** - 319 MB, 11.5K rows → Replaced by party_addresses

**Enrichment Tables:**
- **entity_profiles** - 517 MB, 1.4M rows → Replaced by party_enrichments
- **entity_financials** - 360 KB, 250 rows → Replaced by party_enrichments (JSONB)

**Relationship Tables:**
- **entity_family_relationships** - 259 MB, 0 rows (empty)
- **entity_relationships** - 192 MB, 0 rows (empty)

**archive_metadata** - 32 KB, 8 rows
- Tracks retention periods
- All tables marked for review after 2026-02-20

**Total Archive Size:** 2.4 GB (could be reclaimed)

---

## Audit Schema (4.6 GB)

### Comprehensive Audit Trail

**audit.logged_actions** - 3.3 GB, 1.5M rows
- Full audit log of database changes
- Storage: 3.9 GB total (very large)
- **Recommendation:** Consider archiving old audit logs

**audit.entity_change_log** - 1.3 GB, 52 rows
- Only 52 rows but 1.3 GB? (investigate bloat)
- Storage: Index + TOAST overhead

**audit.catalog_violations** - 80 KB, 0 rows (empty)
**audit.archive_cleanup_log** - 64 KB, 0 rows (empty)

---

## Data Quality Assessment

### Strengths ✅

1. **No Duplicate Identifiers**
   - CPF/CNPJ: 100% unique (0 duplicates)
   - Property codes: Unique constraints enforced

2. **Referential Integrity**
   - No orphaned party records
   - All people/companies have party record
   - 15 foreign key constraints in core schema

3. **Indexing Strategy**
   - 3.2 GB of indexes (39% of core schema)
   - Critical columns indexed (CPF/CNPJ, normalized names, values)

4. **Enrichment Progress**
   - 695K of 1.5M parties enriched (45%)
   - Financial data on 250 enrichments

### Weaknesses ⚠️

1. **Contact Coverage**
   - Only 27% of parties have contact information
   - 73% of parties (1.1M) have no email/phone

2. **Address Coverage**
   - Only 0.75% of parties (11K) have addresses
   - 99% of parties missing address data

3. **Transaction Linkage**
   - 0% of property transactions linked to parties
   - buyer_party_id and seller_party_id columns unused

4. **Verification Rates**
   - Verified contacts: 0.2% (5.5K of 2.6M)
   - Verified addresses: 0.01% (1 of 11K)

5. **Data Freshness**
   - Materialized views may be stale
   - No last_refresh tracking visible

---

## Storage Analysis

### Total Database: 16 GB

**Breakdown by Type:**
| Type | Size | Percentage |
|------|------|------------|
| Table Data | 11.3 GB | 71% |
| Indexes | 4.7 GB | 29% |

**By Schema:**
| Schema | Data | Indexes | Total |
|--------|------|---------|-------|
| core | 5.0 GB | 3.2 GB | 8.2 GB |
| audit | 3.9 GB | 734 MB | 4.6 GB |
| archive | 2.2 GB | 1.2 GB | 3.3 GB |
| analytics | 194 MB | 130 MB | 324 MB |
| others | ~50 MB | ~50 MB | ~100 MB |

### Largest Tables by Total Size

| Table | Total Size | Data | Indexes | Rows |
|-------|------------|------|---------|------|
| audit.logged_actions | 3.3 GB | 3.3 GB | 81 MB | 1.5M |
| audit.entity_change_log | 1.3 GB | 1.3 GB | 109 MB | 52 |
| core.real_estate_properties | 1.3 GB | 930 MB | 370 MB | 1.4M |
| core.mv_entity_enriched | 1.3 GB | 1.1 GB | 141 MB | 0 |
| core.party_contacts | 1.2 GB | 322 MB | 911 MB | 2.6M |
| archive.entities | 951 MB | 681 MB | 270 MB | 1.5M |
| core.mv_contributor_contacts | 918 MB | 743 MB | 176 MB | 0 |
| archive.entity_phones | 724 MB | 451 MB | 273 MB | 1.7M |
| core.parties | 725 MB | 231 MB | 494 MB | 1.5M |

### Largest Indexes

| Index | Table | Size | Type |
|-------|-------|------|------|
| party_contacts_pkey | party_contacts | 347 MB | Primary key |
| idx_party_contacts_value | party_contacts | 273 MB | Contact value lookup |
| uq_party_contact_unique | party_contacts | 181 MB | Uniqueness constraint |
| idx_parties_normalized_name | parties | 177 MB | Name search |
| idx_parties_cpf_cnpj | parties | 154 MB | Identifier lookup |
| ownerships_pkey | ownerships | 149 MB | Primary key |
| parties_pkey | parties | 125 MB | Primary key |

---

## Foreign Key Relationships

### Core Schema Dependencies

**All point to core.parties (good architecture):**

```
core.people → core.parties (party_id)
core.companies → core.parties (party_id)
core.party_contacts → core.parties (party_id)
core.party_addresses → core.parties (party_id)
  └→ core.addresses (address_id)
core.party_enrichments → core.parties (party_id)
core.party_relationships → core.parties (source_party_id, target_party_id)
core.ownerships → core.parties (party_id)
  └→ core.real_estate_properties (property_id)
core.property_transactions → core.parties (buyer_party_id, seller_party_id)
  └→ core.real_estate_properties (property_id)
core.property_ownerships → core.real_estate_properties (property_id)
core.real_estate_properties → core.addresses (address_id)
```

**No foreign keys to archive schema** (good - archive is isolated)

---

## Findings & Recommendations

### Critical Issues

1. **Materialized Views Using Archive Data**
   - `core.mv_entity_enriched` queries archive.entities, archive.entity_emails, archive.entity_phones
   - Should be rebuilt to use core.parties, core.party_contacts
   - **Action:** Rebuild MVs or drop if deprecated

2. **Audit Log Bloat**
   - `audit.entity_change_log`: 1.3 GB for 52 rows (25 MB per row!)
   - Likely TOAST/JSONB bloat
   - **Action:** VACUUM FULL or archive old logs

3. **Low Contact Coverage**
   - 73% of parties have no contact info
   - **Action:** Prioritize enrichment of high-value parties

4. **Transaction-Party Linkage**
   - 0% of transactions linked to parties
   - Columns exist but unused
   - **Action:** Complete backfill or investigate data source

### Optimization Opportunities

1. **Archive Cleanup** (2.4 GB potential reclaim)
   - After 90-day retention period (2026-02-20)
   - Could reduce database size by 15%

2. **Index Optimization**
   - party_contacts has 74% index overhead
   - Review if all indexes are necessary
   - Consider partial indexes for common queries

3. **Materialized View Refresh**
   - No refresh tracking visible
   - May contain stale data
   - **Action:** Implement refresh schedule or remove if unused

4. **Empty Tables Cleanup**
   - `app` schema: 3 empty tables (72 KB)
   - `archive.entity_family_relationships`: 259 MB, 0 rows (bloat)
   - `archive.entity_relationships`: 192 MB, 0 rows (bloat)

### Data Quality Improvements

1. **Verification Campaign**
   - Only 0.2% of contacts verified
   - Implement email/phone verification flow

2. **Address Enrichment**
   - Only 11K of 1.5M parties have addresses (0.75%)
   - Prioritize address collection for active parties

3. **Contact Deduplication**
   - Unique constraint on party_contacts ensures no duplicates
   - Good design

---

## Schema Evolution Observations

### Current State Suggests Recent Migration

Evidence of a **party model migration** from entity-based to party-based architecture:

1. **Dual Structures Present**
   - Active: core.parties, core.party_contacts, core.party_addresses
   - Archived: archive.entities, archive.entity_contacts, archive.entity_addresses
   - Exact row count matches (entities: 1.5M, parties: 1.5M)

2. **Materialized Views Not Updated**
   - Still query archive.entities instead of core.parties
   - Indicates migration is recent/ongoing

3. **Archive Schema Retention**
   - archive_metadata shows retention until 2026-02-20
   - Suggests 90-day safety window

4. **Transaction Linkage Incomplete**
   - buyer_party_id/seller_party_id columns added but not populated
   - Indicates work in progress

### Migration Status: ~95% Complete

✅ **Complete:**
- Party golden record (core.parties)
- Contact unification (core.party_contacts)
- Address linkage (core.party_addresses)
- Enrichment structure (core.party_enrichments)
- Archive isolation (archive schema)

⏳ **Incomplete:**
- Materialized view updates
- Transaction-party linkage
- Archive cleanup (retention period)

---

## Performance Characteristics

### Read Performance

**Well-Indexed Tables:**
- core.parties: 5 indexes (494 MB)
- core.party_contacts: 4 indexes (911 MB)
- core.real_estate_properties: 3+ indexes (370 MB)

**Materialized Views:**
- Pre-aggregated data (2.9 GB)
- Fast reads if refreshed
- Stale if not maintained

### Write Performance

**High Index Overhead:**
- party_contacts: 74% overhead (911 MB indexes / 322 MB data)
- parties: 68% overhead (494 MB / 231 MB)
- May impact insert/update speed

**Audit Trail:**
- Every change logged (3.3 GB audit.logged_actions)
- Additional write overhead

---

## Security & Compliance

### Audit Trail

✅ **Comprehensive logging:**
- 1.5M actions logged
- Full change history preserved

### Data Retention

✅ **Archive management:**
- Metadata tracking (archive.archive_metadata)
- Scheduled cleanup (2026-02-20)

### Access Control

- Foreign key constraints enforced
- Schema isolation (archive read-only)

---

## Summary Statistics

| Metric | Value |
|--------|-------|
| **Total Parties** | 1,536,789 |
| **People** | 1,124,261 (73%) |
| **Companies** | 412,528 (27%) |
| **Contacts** | 2,595,235 |
| **├─ Phones** | 1,722,172 (66%) |
| **└─ Emails** | 873,063 (34%) |
| **Addresses** | 11,530 (0.75% coverage) |
| **Enriched Parties** | 695,310 (45%) |
| **Properties** | 1,447,983 |
| **Transactions** | 1,055,849 |
| **Ownerships** | 1,002,210 |
| **Database Size** | 16 GB |
| **Archive Size** | 3.3 GB (21%) |
| **Audit Size** | 4.6 GB (29%) |
| **Active Data** | 8.2 GB (51%) |

---

## Recommendations Priority Matrix

### High Priority (Do Now)

1. ✅ **Fix Materialized Views** - Rebuild to use core schema
2. ✅ **Investigate Audit Bloat** - 1.3 GB for 52 rows is excessive
3. ⚠️ **Complete Transaction Linkage** - 1M transactions unlinked

### Medium Priority (Next 30 Days)

4. 📊 **Implement MV Refresh Schedule** - Prevent stale data
5. 🧹 **Cleanup Empty Archive Tables** - Reclaim 450 MB (entity_relationships, entity_family_relationships)
6. 📧 **Contact Verification Campaign** - Improve 0.2% verification rate

### Low Priority (Next 90 Days)

7. 🗑️ **Archive Cleanup** - Reclaim 2.4 GB after retention period
8. 📈 **Address Enrichment** - Improve 0.75% coverage
9. 🔍 **Index Review** - Optimize 4.7 GB of indexes

---

**Analysis Date:** 2025-11-22  
**Analyst:** Claude (Independent Review)  
**Database Version:** PostgreSQL 17.5  
**Status:** Production, Actively Used
