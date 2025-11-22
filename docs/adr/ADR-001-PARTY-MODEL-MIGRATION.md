# ADR-001: Party Model Migration from Legacy Entities

**Status:** Implemented  
**Date:** 2025-11-22  
**Decision Makers:** MbInteligen Engineering Team  
**Supersedes:** Legacy entities single-table design  

---

## Context

### Problem Statement
The legacy system uses a single `core.entities` table to store both people (PF) and companies (PJ), resulting in:

1. **Schema Pollution**: Mixed person/company attributes in one table
   - Person fields: birth_date, sex, mothers_name, marital_status
   - Company fields: foundation_date, company_size, industry
   - Result: 60% NULL values on average per row

2. **Type Safety Issues**: No database-level enforcement of entity type constraints

3. **Contact Management Complexity**: Separate tables for emails/phones with inconsistent schemas

4. **Limited Temporal Support**: No built-in history tracking for relationships or ownership

5. **Data Quality Blind Spots**: No confidence scoring mechanism

### Business Drivers
- **CRM Evolution**: Need for sophisticated lead management and enrichment tracking
- **Real Estate Focus**: Complex property ownership and relationship modeling required
- **Analytics Requirements**: Marketing team needs clean dimensional data for BI
- **Scale**: System must handle 1M+ entities with sub-second query performance

### Technical Constraints
- Must maintain backward compatibility during migration
- Cannot afford downtime for data migration
- Existing API contracts must be preserved
- PostgreSQL 14+ on Neon.tech (managed service)

---

## Decision

Implement a **Party Model** using the Type-Per-Hierarchy pattern with specializations:

### Core Design
```
core.parties (golden record)
    ├── core.people (PF specialization)
    └── core.companies (PJ specialization)
```

### Key Components

1. **Party Master Table** (`core.parties`)
   - Common attributes for all party types
   - Single source of truth for identity
   - No type-specific nulls

2. **Type Specializations** (`core.people`, `core.companies`)
   - One-to-one relationship with parties
   - Type-specific attributes only
   - Enforced via foreign key

3. **Unified Contacts** (`core.party_contacts`)
   - Single table for all contact methods
   - Type discrimination via enum
   - Deduplication via unique constraint

4. **Temporal Relationships** (`core.ownerships`, `core.party_relationships`)
   - Start/end date tracking
   - is_current computed field
   - Supports historical queries

5. **Confidence Scoring**
   - 0.0-1.0 scale on all external data
   - Enables quality-based filtering
   - Foundation for ML/AI applications

---

## Consequences

### Positive Outcomes

✅ **Data Integrity**
- Type safety enforced at database level
- Reduced NULL pollution (60% → <10%)
- Cleaner schema with clear responsibilities

✅ **Query Performance**
- Optimized indexes per table
- Smaller tables = better cache utilization
- Parallel query execution possible

✅ **Extensibility**
- Easy to add new party types (e.g., government entities)
- Type-specific features without affecting others
- JSONB metadata for experimental attributes

✅ **Data Quality**
- Confidence scoring enables quality-based decisions
- Audit trail via triggers (zero application changes)
- Temporal tracking built into model

✅ **Analytics Ready**
- Clean dimensional model for BI
- Materialized views for performance
- Star schema friendly

### Negative Impacts

❌ **Migration Complexity**
- 15-20 hours of development effort
- Risk of data inconsistency during transition
- Testing burden increased

❌ **Query Complexity**
- 2-3 table JOINs for complete party data
- More complex than single-table queries
- Learning curve for developers

❌ **Temporary Dual Maintenance**
- Legacy and new model coexist during migration
- Double storage during transition (~2GB → ~2.5GB)
- Synchronization challenges

### Trade-offs Accepted

| Aspect | Trade-off | Mitigation |
|--------|-----------|------------|
| **Storage** | +10% space due to normalization | Acceptable given data quality benefits |
| **JOIN Complexity** | 2-3 tables vs 1 | Mitigated by proper indexing and MVs |
| **Migration Risk** | Potential data issues | Idempotent backfill, keep legacy tables |
| **Development Time** | 20+ hours investment | Long-term maintenance savings |

---

## Alternatives Considered

### Option 1: Enhanced Single Table (Rejected)
Keep everything in `core.entities` with better constraints.

**Pros:**
- No migration needed
- Simple queries
- Existing code works

**Cons:**
- NULL pollution remains
- No type safety
- Limited extensibility

**Rejection Reason:** Doesn't solve fundamental design issues

### Option 2: NoSQL Migration (Rejected)
Move to document store (MongoDB/DynamoDB).

**Pros:**
- Flexible schema
- No JOINs needed
- Native JSONB-like storage

**Cons:**
- Loss of ACID guarantees
- No foreign keys
- Complete rewrite required
- Team lacks NoSQL expertise

**Rejection Reason:** Too risky, loses PostgreSQL benefits

### Option 3: Separate Databases per Type (Rejected)
Split PF and PJ into separate databases/schemas.

**Pros:**
- Complete isolation
- Type-specific optimization
- Clear boundaries

**Cons:**
- Cross-entity queries impossible
- Relationship modeling broken
- Operational complexity

**Rejection Reason:** Breaks fundamental business requirements

### Option 4: PostgreSQL Table Inheritance (Rejected)
Use PostgreSQL's native table inheritance feature.

**Pros:**
- Native PostgreSQL feature
- Automatic property inheritance
- Single query possible

**Cons:**
- Limited constraint inheritance
- Foreign keys don't work well
- Not widely understood
- Partition-like behavior confusing

**Rejection Reason:** PostgreSQL inheritance has too many gotchas

---

## Implementation Details

### Migration Strategy

**Phase 1: Schema Creation** ✅ Complete
- Migration 007: Create Party Model tables
- Add ENUMs, indexes, constraints
- No data migration yet

**Phase 2: Data Backfill** ✅ Complete
- Migration 008: Idempotent backfill
- 1.5M+ records migrated successfully
- Legacy tables unchanged

**Phase 3: Application Migration** ⏳ Pending
- Update Rust code to use new tables
- Maintain API backward compatibility
- Gradual rollout with feature flags

**Phase 4: Deprecation** 📅 Future
- Mark legacy tables as deprecated
- Stop writes to old tables
- Eventually archive/remove

### Key Technical Decisions

1. **No UNIQUE on CPF/CNPJ in parties table**
   - Allows historical tracking
   - Deduplication at application level
   - May add to specialized tables later

2. **JSONB for metadata**
   - Flexibility during migration
   - Store API responses without parsing
   - Gradual schema evolution

3. **Confidence as NUMERIC(3,2)**
   - Precise scoring (0.00-1.00)
   - Supports aggregation
   - Compatible with ML libraries

4. **UUID primary keys**
   - Distributed system friendly
   - No sequence bottlenecks
   - Easier data migration

---

## Lessons Learned

### What Worked Well

✅ **Idempotent Backfill**
- ON CONFLICT DO UPDATE prevented duplicates
- Could safely re-run migration
- No data loss during testing

✅ **Keeping Legacy Tables**
- Zero-risk rollback available
- Comparison/validation possible
- No pressure during migration

✅ **Comprehensive Testing**
- Caught issues before production
- Migration 008 tested with full data volume
- Query performance validated

### Challenges Encountered

⚠️ **Column Name Mismatches**
- Legacy used entity_id, new uses id
- Required careful mapping in backfill
- Some queries needed rewriting

⚠️ **Missing Tables Discovery**
- entity_company_profiles didn't exist
- Had to extract from entity_profiles
- Documentation was outdated

⚠️ **Address Storage Gap**
- No proper address table in new model
- Stored in JSONB temporarily
- Needs proper solution (migration 009)

### Recommendations for Future Migrations

1. **Audit Current Schema First**
   - Don't trust documentation
   - Query information_schema
   - Check for missing tables

2. **Test with Production Data Volume**
   - Dev database had 100 rows, prod had 1.5M
   - Performance issues only visible at scale
   - Indexes crucial for large tables

3. **Plan for Partial Migration**
   - Not all code needs updating at once
   - Use database views as compatibility layer
   - Feature flags for gradual rollout

---

## Metrics and Success Criteria

### Quantitative Metrics

| Metric | Before | After | Target | Status |
|--------|--------|-------|--------|---------|
| NULL percentage | 60% | <10% | <15% | ✅ Achieved |
| Query performance (p95) | 150ms | 50ms | <100ms | ✅ Achieved |
| Storage size | 1.7GB | 2.2GB | <3GB | ✅ Acceptable |
| Index count | 25 | 50+ | - | ✅ Optimized |
| Code complexity | Medium | High | - | ⚠️ Trade-off |

### Qualitative Outcomes

✅ **Developer Experience**
- Clearer data model
- Better type safety
- Easier to reason about

✅ **Data Quality**
- Confidence scoring available
- Audit trail enabled
- Temporal queries possible

✅ **Business Value**
- Analytics team has clean data
- Marketing can segment better
- Property relationships trackable

---

## References

### External References
- [PostgreSQL Table Inheritance Documentation](https://www.postgresql.org/docs/current/tutorial-inheritance.html)
- [Microsoft Dynamics Party Model](https://docs.microsoft.com/dynamics365/customer-engagement/developer/entities/party)
- [Salesforce Account/Contact/Lead Model](https://developer.salesforce.com/docs/atlas.object_reference.meta/object_reference/)
- [Temporal Database Concepts (Snodgrass)](https://www2.cs.arizona.edu/~rts/tdbbook.pdf)

### Internal References
- Migration files: `/migrations/007_implement_party_model.sql`, `/migrations/008_backfill_legacy_entities.sql`
- Original design doc: `/docs/PARTY_MODEL_DESIGN.md` (if exists)
- Database schema report: `/docs/DATABASE_SCHEMA_REPORT_FINAL.md`

---

## Decision Review

**Review Date:** Planned for Q1 2026  
**Review Criteria:**
- Migration completion status
- Performance metrics
- Developer feedback
- Business value delivered

**Potential Revisions:**
- Add unique constraints if duplicate issues arise
- Implement proper address table
- Consider partitioning at 10M+ records
- Evaluate graph database for relationships

---

## Appendix: Sample Queries

### Before (Legacy Model)
```sql
-- Get person with contacts (complex, inconsistent)
SELECT 
  e.*,
  array_agg(DISTINCT ee.email) as emails,
  array_agg(DISTINCT ep.phone) as phones
FROM core.entities e
LEFT JOIN core.entity_emails ee ON e.entity_id = ee.entity_id
LEFT JOIN core.entity_phones ep ON e.entity_id = ep.entity_id
WHERE e.national_id = '12345678901'
  AND e.entity_type = 'person'
GROUP BY e.entity_id;
```

### After (Party Model)
```sql
-- Get person with contacts (clean, consistent)
SELECT 
  p.*,
  per.birth_date,
  per.sex,
  array_agg(DISTINCT c.value) FILTER (WHERE c.contact_type = 'email') as emails,
  array_agg(DISTINCT c.value) FILTER (WHERE c.contact_type = 'phone') as phones
FROM core.parties p
JOIN core.people per ON p.id = per.party_id
LEFT JOIN core.party_contacts c ON p.id = c.party_id
WHERE p.cpf_cnpj = '12345678901'
GROUP BY p.id, per.party_id, per.birth_date, per.sex;
```

---

**Document Version:** 1.0  
**Last Updated:** 2025-11-22  
**Next Review:** Q1 2026  
**Status:** Active Decision