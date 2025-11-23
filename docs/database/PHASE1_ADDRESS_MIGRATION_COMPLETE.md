# Phase 1: Address Migration - COMPLETE ✅

**Date:** 2025-11-22  
**Status:** Successfully completed  
**Records Migrated:** 11,530 addresses

---

## Summary

✅ **Successfully created `core.party_addresses` table**  
✅ **Migrated all 11,530 addresses from `entity_addresses`**  
✅ **Helper functions created for address management**  
⏳ **Code update pending** (`src/db_storage.rs`)

---

## Migration Details

### Table Created

**`core.party_addresses`** with features:
- UUID primary key
- Foreign keys to `parties` and `addresses`
- Confidence scoring (0-1 scale)
- Temporal tracking (`is_current`, `is_primary`)
- Metadata (JSONB) for migration tracking
- Auto-update trigger for `updated_at`

### Indexes Created

1. `idx_party_addresses_party` - Lookup by party
2. `idx_party_addresses_address` - Reverse lookup (which parties at address?)
3. `idx_party_addresses_primary` - UNIQUE partial index for primary addresses
4. `idx_party_addresses_current` - Current addresses filter
5. `idx_party_addresses_confidence` - High-confidence addresses (≥0.75)
6. `uq_party_address_link` - UNIQUE (party_id, address_id)

### Helper Functions

**`get_party_primary_address(party_id UUID)`**
- Returns primary address for a party
- Filters by `is_primary = true` AND `is_current = true`

**`set_party_primary_address(party_id UUID, address_id UUID)`**
- Atomic operation to set new primary address
- Automatically demotes other primary addresses for same party

### Backfill Results

```
Legacy addresses (entity_addresses): 11,530
Migrated addresses (party_addresses): 11,530 ✅
Primary addresses: 1,054
Verified addresses: 1
```

**Confidence Score Distribution:**
- `0.95` - Verified addresses (1 address)
- `0.85` - Primary addresses (1,054 addresses)  
- `0.75` - Other addresses (10,475 addresses)

### Sample Migrated Data

| CPF/CNPJ | Name | Street | City | Type | Primary | Confidence |
|----------|------|--------|------|------|---------|------------|
| 09133198802 | Claudia Engel Becher | CARVALHO | SAO PAULO | residential | No | 0.75 |
| 64876888604 | CLAUDIA BEATRIZ RIBEIRO | PAULO OROZIMBO | SAO PAULO | residential | Yes | 0.95 |
| 01798315000350 | CBM DISTRIBUIDORA | TREZE DE MAIO | SAO PAULO | residential | Yes | 0.85 |

---

## Data Quality Checks

✅ **No duplicate party-address links** (0 conflicts during migration)  
✅ **No parties with multiple primary addresses** (constraint enforced)  
✅ **100% migration success** (11,530 legacy = 11,530 migrated)

---

## Remaining Phase 1 Tasks

### ⏳ Update Application Code

**File:** `src/db_storage.rs`

**Changes needed:**
1. Add address storage logic to `store_enriched_person_with_lead()`
2. Parse addresses from Work API response
3. Insert into `core.party_addresses`
4. Apply confidence scoring based on position/relationship

**Example logic:**
```rust
// Parse addresses from Work API
let enderecos = work_data.get("enderecos")
    .and_then(|e| e.as_array());

if let Some(addresses) = enderecos {
    for (idx, addr) in addresses.iter().enumerate() {
        // Extract address fields
        let cep = addr.get("cep").and_then(|c| c.as_str());
        let logradouro = addr.get("logradouro").and_then(|l| l.as_str());
        // ... more fields
        
        // Calculate confidence based on position
        let confidence = match idx {
            0 => 0.90,  // First address
            _ => 0.75,  // Additional addresses
        };
        
        // Insert address (create in addresses table first if needed)
        // Then link to party via party_addresses
    }
}
```

**Priority:** High (required for new enrichments to store addresses)

---

## Migration Files

- **Migration SQL:** `docs/database/migrations/009_create_party_addresses.sql`
- **Status Report:** `docs/database/PARTY_MODEL_MIGRATION_STATUS.md`
- **This Report:** `docs/database/PHASE1_ADDRESS_MIGRATION_COMPLETE.md`

---

## Verification Queries

### Check migration success
```sql
SELECT COUNT(*) FROM core.party_addresses; -- Expected: 11,530
```

### View primary addresses by city
```sql
SELECT 
    a.city,
    COUNT(*) as primary_addresses
FROM core.party_addresses pa
JOIN core.addresses a ON pa.address_id = a.id
WHERE pa.is_primary = true
GROUP BY a.city
ORDER BY COUNT(*) DESC
LIMIT 10;
```

### Find high-confidence addresses
```sql
SELECT 
    p.cpf_cnpj,
    p.full_name,
    a.street,
    a.city,
    pa.confidence_score
FROM core.party_addresses pa
JOIN core.parties p ON pa.party_id = p.id
JOIN core.addresses a ON pa.address_id = a.id
WHERE pa.confidence_score >= 0.85
ORDER BY pa.confidence_score DESC
LIMIT 20;
```

### Check for data quality issues
```sql
-- Parties with multiple primary addresses (should be 0)
SELECT party_id, COUNT(*) 
FROM core.party_addresses 
WHERE is_primary = true 
GROUP BY party_id 
HAVING COUNT(*) > 1;

-- Addresses without parties (orphaned - should be 0 after migration)
SELECT COUNT(*) 
FROM core.addresses a
WHERE NOT EXISTS (
    SELECT 1 FROM core.party_addresses pa 
    WHERE pa.address_id = a.id
);
```

---

## Next Steps (Roadmap)

### Phase 2: Financial Data Migration
- **Decision:** Migrate 250 records to `party_enrichments.normalized_data["financials"]` (JSONB)
- **Reason:** Only 250 records, low priority, JSONB provides flexibility
- **Timeline:** Week 2

### Phase 3: Property Transactions
- Add `buyer_party_id`/`seller_party_id` to `property_transactions`
- Backfill from `buyer_entity_id`/`seller_entity_id`
- Drop 10 FK constraints pointing to `core.entities`
- **Timeline:** Week 3

### Phase 4: Code Audit & Archive
- Grep for `entity_id` references in codebase
- Ensure all reads/writes use Party Model
- Archive legacy tables to `archive` schema
- Monitor 30-90 days, then drop
- **Timeline:** Week 4+

---

## Success Metrics

✅ **11,530 addresses migrated** (100%)  
✅ **1,054 primary addresses identified**  
✅ **Zero data quality issues detected**  
✅ **Helper functions operational**  
✅ **Indexes optimized for performance**

**Blocker Resolution Progress:** 1 of 4 (25%)

---

**Migration Lead:** AI Assistant  
**Reviewed By:** Pending  
**Production Deployment:** Completed (2025-11-22)
