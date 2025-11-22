# Database Storage Analysis - Work API Enrichment Data
## ✅ UPDATED WITH ACTUAL DATABASE SCHEMA

## Executive Summary

**GOOD NEWS:** The production database has a **comprehensive, enterprise-grade schema** that can store **ALL Work API enrichment data** with minimal to NO schema changes needed!

The database uses an Entity-Attribute-Value (EAV) pattern with specialized tables, far more advanced than the init.sql suggested.

---

## Current Production Schema (Actual Tables Found)

### Core Tables
1. **`core.entities`** - Main entity table (people/companies) ✅
2. **`core.entity_profiles`** - Demographic/profile data ✅
3. **`core.entity_financials`** - Financial/economic data ✅
4. **`core.entity_emails`** - Email addresses ✅
5. **`core.entity_phones`** - Phone numbers ✅
6. **`core.entity_addresses`** - Address junction table ✅
7. **`app.addresses`** - Address details ✅
8. **`core.entity_relationships`** - Family/business relationships ✅
9. **`core.entity_attributes`** - Flexible key-value attributes (partitioned) ✅

### Specialized Attribute Tables (Partitioned from entity_attributes)
- `core.entity_attributes_benefit` - Government benefits
- `core.entity_attributes_education` - Education history
- `core.entity_attributes_interest` - Interests
- `core.entity_attributes_purchase` - Purchase history
- `core.entity_attributes_vehicle` - Vehicle ownership
- `core.entity_attributes_other` - Misc attributes

---

## Work API Data Mapping - COMPLETE COVERAGE ✅

### 1. DadosBasicos (Basic Personal Data)

| Work API Field | Database Mapping | Status |
|----------------|------------------|--------|
| cpf | `core.entities.national_id` | ✅ PERFECT |
| nome | `core.entities.name` | ✅ PERFECT |
| sexo | `core.entity_profiles.sex` | ✅ PERFECT |
| dataNascimento | `core.entity_profiles.birth_date` | ✅ PERFECT |
| nomeMae | `metadata` or new field | ⚠️ STORE IN METADATA |
| nomePai | `metadata` or new field | ⚠️ STORE IN METADATA |
| escolaridade | `core.entity_profiles.education_level` | ✅ PERFECT |
| estadoCivil | `core.entity_profiles.marital_status` | ✅ PERFECT |
| nacionalidade | `core.entity_profiles.nationality` | ✅ PERFECT |
| municipioNascimento | `core.entity_profiles.metadata` | ✅ USE METADATA |
| cor | `core.entity_profiles.metadata` | ✅ USE METADATA |
| cns | `core.entity_profiles.metadata` | ✅ USE METADATA |
| obito.dataObito | `core.entity_profiles.death_date` | ✅ PERFECT |
| situacaoCadastral | `core.entity_profiles.metadata` | ✅ USE METADATA |
| conjuge | `core.entity_relationships` | ✅ PERFECT |

### 2. DadosEconomicos (Economic/Financial Data)

| Work API Field | Database Mapping | Status |
|----------------|------------------|--------|
| renda | `core.entity_financials.reported_income` | ✅ PERFECT |
| score.scoreCSBA | `core.entity_financials.credit_score` | ✅ PERFECT |
| score.scoreCSBAFaixaRisco | `core.entity_financials.risk_score` or metadata | ✅ PERFECT |
| poderAquisitivo.* | `core.entity_financials.metadata` | ✅ USE METADATA |
| serasaMosaic.* | `core.entity_financials.metadata` | ✅ USE METADATA |

**Mapping Example:**
```json
{
  "financial_year": 2025,
  "reported_income": 5708.71,  // renda * 1.9
  "credit_score": 995,         // scoreCSBA
  "risk_score": 0.1,           // mapped from "BAIXISSIMO RISCO"
  "source": "work_api",
  "confidence": "high",
  "metadata": {
    "poder_aquisitivo": {
      "codigo": "4",
      "descricao": "MEDIO",
      "faixa_renda": "De R$ 3097.00 até R$ 7755.80"
    },
    "mosaic": {
      "codigo_novo": "A01",
      "descricao": "Ricos e influentes",
      "classe": "Elites Brasileiras"
    }
  }
}
```

### 3. Emails

| Work API Field | Database Mapping | Status |
|----------------|------------------|--------|
| email | `core.entity_emails.email` | ✅ PERFECT |
| prioridade | `core.entity_emails.metadata.priority` | ✅ USE METADATA |
| qualidade | `core.entity_emails.metadata.quality` | ✅ USE METADATA |
| emailPessoal | `core.entity_emails.email_type` | ✅ PERFECT |
| blacklist | `core.entity_emails.metadata.blacklist` | ✅ USE METADATA |

**Mapping Example:**
```json
{
  "email": "exemplo@email.com",
  "email_type": "personal",  // based on emailPessoal
  "is_primary": true,        // based on prioridade
  "is_verified": true,       // based on qualidade == "BOM"
  "metadata": {
    "prioridade": "MUITO ALTA",
    "qualidade": "BOM",
    "blacklist": "NÃO"
  }
}
```

### 4. Telefones (Phones)

| Work API Field | Database Mapping | Status |
|----------------|------------------|--------|
| telefone | `core.entity_phones.phone` | ✅ PERFECT |
| tipo | `core.entity_phones.phone_type` | ✅ PERFECT |
| whatsapp | `core.entity_phones.is_whatsapp` | ✅ PERFECT |
| operadora | `core.entity_phones.carrier` | ✅ PERFECT |
| status | `core.entity_phones.metadata.status` | ✅ USE METADATA |

### 5. Endereços (Addresses)

| Work API Field | Database Mapping | Status |
|----------------|------------------|--------|
| tipoLogradouro | `app.addresses.street_type` | ✅ PERFECT |
| logradouro | `app.addresses.street` | ✅ PERFECT |
| logradouroNumero | `app.addresses.number` | ✅ PERFECT |
| complemento | `app.addresses.complement` | ✅ PERFECT |
| bairro | `app.addresses.neighborhood` | ✅ PERFECT |
| cidade | `app.addresses.city` | ✅ PERFECT |
| uf | `app.addresses.state` | ✅ PERFECT |
| cep | `app.addresses.zip_code` | ✅ PERFECT |

**Junction table:** `core.entity_addresses` with ranking, is_primary, etc.

### 6. Empresas (Company Relationships)

| Work API Field | Database Mapping | Status |
|----------------|------------------|--------|
| cnpj | Target entity in `core.entities` | ✅ PERFECT |
| tipoRelacao | `core.entity_relationships.relationship_type` | ✅ PERFECT |
| relacao | `core.entity_relationships.metadata.role` | ✅ PERFECT |
| admissao | `core.entity_relationships.valid_from` | ✅ PERFECT |
| demissao | `core.entity_relationships.valid_until` | ✅ PERFECT |

**Example:**
```json
{
  "source_entity_id": "person_uuid",
  "target_entity_id": "company_uuid",
  "relationship_category": "professional",
  "relationship_type": "ownership",
  "valid_from": "1990-03-28",
  "valid_until": "2018-10-04",
  "is_current": false,
  "metadata": {
    "tipo_relacao": "QSA",
    "relacao": "OWNER"
  }
}
```

### 7. Parentes (Relatives/Family)

| Work API Field | Database Mapping | Status |
|----------------|------------------|--------|
| nomeParente | Target entity in `core.entities` | ✅ PERFECT |
| cpfParente | Target entity `national_id` | ✅ PERFECT |
| grauParentesco | `core.entity_relationships.relationship_type` | ✅ PERFECT |

**Example:**
```json
{
  "relationship_category": "family",
  "relationship_type": "son",  // from "FILHA(O)"
  "is_current": true
}
```

### 8. Profissão (Profession)

| Work API Field | Database Mapping | Status |
|----------------|------------------|--------|
| cboDescricao | `core.entity_profiles.occupation` | ✅ PERFECT |
| cbo | `core.entity_profiles.metadata.cbo_code` | ✅ USE METADATA |
| pis | `core.entity_profiles.metadata.pis` | ✅ USE METADATA |

### 9. Other Data

| Work API Data | Database Mapping | Status |
|---------------|------------------|--------|
| **beneficios** | `core.entity_attributes_benefit` | ✅ PERFECT TABLE EXISTS |
| **comprasId** | `core.entity_attributes_purchase` | ✅ PERFECT TABLE EXISTS |
| **DadosImposto** | `core.entity_financials.metadata` or attributes | ✅ USE METADATA |
| **perfilConsumo** | `core.entity_attributes_interest` | ✅ PERFECT TABLE EXISTS |
| **imunoBiologicos** | `core.entity_profiles.metadata.vaccinations` | ✅ USE METADATA |
| **vizinhos** | `core.entity_relationships` (category: neighbor) | ✅ USE RELATIONSHIPS |

---

## Implementation Strategy

### Phase 1: Core Entity & Profile (EASIEST - 2-3 hours)

**What to store:**
1. Create/Update entity in `core.entities`
   - `national_id` = CPF
   - `name` = nome
   - `entity_type` = 'person'
   - `is_enriched` = true
   - `enriched_at` = now()
   - `data_source` = 'work_api'

2. Create/Update profile in `core.entity_profiles`
   - `sex`, `birth_date`, `death_date`
   - `nationality`, `marital_status`, `education_level`
   - `occupation`
   - `metadata` = {nomeMae, nomePai, cor, cns, municipioNascimento, etc.}

**Code needed:**
- Upsert entity by `national_id`
- Upsert profile by `entity_id`
- Transaction handling

### Phase 2: Financial Data (EASY - 1-2 hours)

**What to store:**
1. Insert into `core.entity_financials`
   - `financial_year` = current year
   - `reported_income` = renda * 1.9
   - `credit_score` = scoreCSBA
   - `risk_score` = map risk level to numeric
   - `source` = 'work_api'
   - `metadata` = {poder_aquisitivo, mosaic, all other economic data}

**Code needed:**
- Upsert by `(entity_id, financial_year, NULL)`
- JSON serialization for metadata

### Phase 3: Contact Info (MEDIUM - 2-3 hours)

**What to store:**
1. Emails → `core.entity_emails`
   - Map priority to `is_primary`
   - Map quality to `is_verified`
   - Store all metadata

2. Phones → `core.entity_phones`
   - Parse phone type
   - Set `is_whatsapp`
   - Store carrier
   - Set `is_primary` based on ranking

**Code needed:**
- Dedup logic (emails/phones may already exist)
- Ranking algorithm
- Primary selection logic

### Phase 4: Addresses (MEDIUM - 2-3 hours)

**What to store:**
1. Addresses → `app.addresses`
   - Full address details
   - Generate address hash for deduplication

2. Link via `core.entity_addresses`
   - Set first as `is_primary`
   - Set all as `is_current`
   - Set `data_source` = 'work_api'

**Code needed:**
- Address normalization
- Hash generation for dedup
- Geocoding (optional, can be async)

### Phase 5: Relationships (COMPLEX - 4-6 hours)

**What to store:**
1. Companies → Create entities + relationships
   - Create company entity if not exists (by CNPJ)
   - Create relationship with dates
   
2. Family → Create entities + relationships
   - Create relative entity if not exists (by CPF)
   - Create family relationship

**Code needed:**
- Recursive entity creation
- Relationship type mapping
- Date parsing and validation

---

## Schema Changes Needed

### MINIMAL (Recommended for MVP):

**None!** The schema is already perfect for 95% of the data.

### OPTIONAL Enhancements:

1. **Add parent name fields to `core.entity_profiles`:**
```sql
ALTER TABLE core.entity_profiles 
ADD COLUMN mother_name text,
ADD COLUMN father_name text;
```

2. **Add risk score enum** (instead of using numeric):
```sql
CREATE TYPE core.risk_level AS ENUM (
  'BAIXISSIMO_RISCO',
  'BAIXO_RISCO', 
  'MEDIO_RISCO',
  'ALTO_RISCO',
  'ALTISSIMO_RISCO'
);

ALTER TABLE core.entity_financials
ADD COLUMN risk_level core.risk_level;
```

---

## Data Flow Implementation

```rust
// Pseudo-code
async fn save_work_api_enrichment(
    cpf: &str,
    work_data: &WorkApiCompleteResponse
) -> Result<Uuid> {
    let mut tx = db.begin().await?;
    
    // 1. Upsert entity
    let entity_id = upsert_entity(&mut tx, cpf, &work_data).await?;
    
    // 2. Upsert profile
    upsert_profile(&mut tx, entity_id, &work_data).await?;
    
    // 3. Upsert financials
    upsert_financials(&mut tx, entity_id, &work_data).await?;
    
    // 4. Upsert emails
    upsert_emails(&mut tx, entity_id, &work_data).await?;
    
    // 5. Upsert phones
    upsert_phones(&mut tx, entity_id, &work_data).await?;
    
    // 6. Upsert addresses
    upsert_addresses(&mut tx, entity_id, &work_data).await?;
    
    // 7. Upsert relationships (companies, family)
    upsert_relationships(&mut tx, entity_id, &work_data).await?;
    
    tx.commit().await?;
    Ok(entity_id)
}
```

---

## Storage Estimation

Per enriched person (using actual schema):
- `core.entities`: ~300 bytes
- `core.entity_profiles`: ~500 bytes
- `core.entity_financials`: ~400 bytes
- `app.addresses` (avg 2): ~600 bytes
- `core.entity_addresses` (avg 2): ~200 bytes
- `core.entity_emails` (avg 3): ~300 bytes
- `core.entity_phones` (avg 5): ~400 bytes
- `core.entity_relationships` (avg 5): ~500 bytes
- **Total: ~3.2 KB per person**

**For scale:**
- 100K people = ~320 MB
- 1M people = ~3.2 GB
- 10M people = ~32 GB

**Conclusion:** Storage is NOT a concern. Neon DB can easily handle this.

---

## Key Advantages of Current Schema

✅ **Enterprise-grade design** with proper normalization
✅ **Audit trails** built-in (triggers on entities/relationships)
✅ **Full-text search** support (tsvector on entities)
✅ **Flexible metadata** (JSONB for extensibility)
✅ **Temporal relationships** (valid_from/valid_until)
✅ **Confidence scoring** built-in
✅ **Multi-source support** (data_source enum)
✅ **Partitioned attributes** for performance
✅ **Proper indexes** for all queries

---

## Recommended Next Steps

1. **Phase 1 (MVP):** Implement core entity + profile + financials storage
   - Estimated: 4-6 hours
   - Impact: HIGH (stores most valuable data)

2. **Phase 2:** Add contact info (emails/phones)
   - Estimated: 2-3 hours
   - Impact: HIGH (enables communication)

3. **Phase 3:** Add addresses
   - Estimated: 2-3 hours
   - Impact: MEDIUM (location data)

4. **Phase 4:** Add relationships
   - Estimated: 4-6 hours
   - Impact: MEDIUM (network analysis)

**Total MVP:** 8-12 hours of development

---

## Risk Assessment

**LOW RISK:**
- Schema is already perfect ✅
- No breaking changes needed ✅
- Can implement incrementally ✅
- Transaction support built-in ✅
- Rollback capability exists ✅

**MEDIUM RISK:**
- Duplicate detection/merging
- Data quality validation
- Performance at scale (but indexes exist)

**MITIGATION:**
- Use UPSERT with conflict resolution
- Validate data before insertion
- Monitor query performance
- Use batch operations where possible
