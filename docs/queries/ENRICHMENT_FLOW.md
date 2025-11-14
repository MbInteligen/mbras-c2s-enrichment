# Work API Enrichment Database Flow

This document explains how Work API data flows into our PostgreSQL database.

## Overview

When we receive data from Work API, we update **6 main database tables**:

```
Work API Response
       ↓
┌──────────────────┐
│  1. core.entities │  ← Create/update person entity
└──────────────────┘
       ↓
┌─────────────────────────┐
│ 2. core.entity_profiles │  ← Store demographic data
└─────────────────────────┘
       ↓
┌──────────────────────────────┐
│ 3. core.entity_financials    │  ← Store income & credit score
└──────────────────────────────┘
       ↓
┌────────────────────────┐
│ 4. core.entity_emails  │  ← Store emails
└────────────────────────┘
       ↓
┌────────────────────────┐
│ 5. core.entity_phones  │  ← Store phones
└────────────────────────┘
       ↓
┌────────────────────────┐    ┌──────────────────┐
│ 6. core.entity_addresses│ ←→ │ app.addresses    │
└────────────────────────┘    └──────────────────┘
```

## Table Updates

### 1. core.entities
**Purpose**: Main person entity record

**Logic**:
- If CPF exists: UPDATE with name and enrichment flag
- If CPF doesn't exist: INSERT new entity

**Fields Updated**:
- `name` - Full name from Work API
- `canonical_name` - UPPERCASE version for matching
- `is_enriched` - Set to `true`
- `enriched_at` - Current timestamp
- `entity_type` - Always `'person'`
- `data_source` - Always `'api'`

### 2. core.entity_profiles
**Purpose**: Demographic information

**Logic**: UPSERT (insert or update if exists)

**Fields Stored**:
| Field | Source (Work API) | Transformation |
|-------|-------------------|----------------|
| `sex` | `DadosBasicos.sexo` | First char only: "M - MASCULINO" → "M" |
| `birth_date` | `DadosBasicos.dataNascimento` | "29/04/1975" → "1975-04-29" |
| `nationality` | `DadosBasicos.nacionalidade` | Direct |
| `marital_status` | `DadosBasicos.estadoCivil` | Direct |
| `education_level` | `DadosBasicos.escolaridade` | Direct |
| `metadata` | Various | JSON with: mother_name, father_name, cor, municipio_nascimento, cns |

### 3. core.entity_financials
**Purpose**: Financial data (yearly)

**Logic**: Check if record exists for current year, then UPDATE or INSERT

**Fields Stored**:
| Field | Source (Work API) | Transformation |
|-------|-------------------|----------------|
| `reported_income` | `DadosEconomicos.renda` | **Multiplied by 1.9x** |
| `credit_score` | `DadosEconomicos.score.scoreCSBA` | Direct (integer) |
| `risk_score` | `DadosEconomicos.score.scoreCSBAFaixaRisco` | Mapped to 0.1-0.9 scale |
| `financial_year` | - | Current year |
| `metadata` | Various | JSON with: poder_aquisitivo, serasaMosaic |

**Risk Score Mapping**:
```
BAIXISSIMO RISCO → 0.1
BAIXO RISCO      → 0.3
MEDIO RISCO      → 0.5
ALTO RISCO       → 0.7
ALTISSIMO RISCO  → 0.9
```

### 4. core.entity_emails
**Purpose**: Email addresses

**Logic**: INSERT each email, ignore duplicates

**Fields Stored**:
| Field | Value | Notes |
|-------|-------|-------|
| `email` | Work API email | **Lowercased** |
| `email_type` | `'personal'` | Always personal |
| `is_primary` | First email = true | Index 0 is primary |
| `is_verified` | `qualidade == "BOM"` | Boolean |
| `metadata` | JSON | prioridade, qualidade, email_pessoal, blacklist |

### 5. core.entity_phones
**Purpose**: Phone numbers

**Logic**: INSERT each phone, ignore duplicates

**Fields Stored**:
| Field | Source | Transformation |
|-------|--------|----------------|
| `phone` | Work API telefone | Raw number |
| `phone_type` | Work API tipo | "MÓVEL" → 'mobile', "RESIDENCIAL" → 'landline' |
| `is_primary` | First phone = true | Index 0 is primary |
| `is_whatsapp` | whatsapp == "SIM" | Boolean |
| `carrier` | operadora | Direct |
| `metadata` | JSON | status field |

### 6. Addresses (Two Tables)
**Purpose**: Physical addresses

**Logic**: Two-step process
1. INSERT into `app.addresses` (address details)
2. INSERT into `core.entity_addresses` (link to entity)

**app.addresses**:
- `street_type` - "AV", "RUA", etc.
- `street` - Street name
- `number` - Street number
- `complement` - Apartment, etc.
- `neighborhood` - Bairro
- `city` - Cidade
- `state` - UF (e.g., "SP")
- `zip_code` - CEP

**core.entity_addresses** (link table):
- `entity_id` - Links to person
- `address_id` - Links to address
- `address_type` - Always `'residential'`
- `is_primary` - True for first address
- `is_current` - Always `true`

## Key Behaviors

### Deduplication Strategy
- **Entities**: Checked by `national_id` (CPF)
- **Emails/Phones/Addresses**: Duplicates silently ignored (no error)

### Primary Selection
First item in each array is marked as `is_primary = true`:
- First email → primary email
- First phone → primary phone
- First address → primary address

### Data Quality
- **Missing fields**: Handled with `COALESCE()` - keeps existing value
- **Invalid data**: Silently skipped (no error handling for individual items)
- **Confidence**: All Work API data marked as `'high'` confidence

### Special Transformations
1. **Income**: Multiplied by 1.9x (adjustment factor)
2. **Dates**: Brazilian format (DD/MM/YYYY) → PostgreSQL format (YYYY-MM-DD)
3. **Names**: Canonical name stored in uppercase for matching
4. **Emails**: Normalized to lowercase

## Performance Considerations

### Sequential Queries
The implementation uses **sequential queries** instead of complex CTEs:
- Better compatibility with SQLx
- Easier error handling
- Clear transaction boundaries

### Batch Processing
When processing multiple CPFs (e.g., from CEP lookup):
- Recommended delay: **3 seconds** between requests
- Prevents Work API timeouts
- See: `docs/integrations/WORK_API_RATE_LIMITING.md`

## Example Data Flow

```json
{
  "DadosBasicos": {
    "nome": "EDSON TARRAF",
    "dataNascimento": "28/01/1958",
    "sexo": "M - MASCULINO",
    "nomeMae": "OLINDA ADDAS TARRAF"
  },
  "DadosEconomicos": {
    "renda": "20720,00",
    "score": {
      "scoreCSBA": "750",
      "scoreCSBAFaixaRisco": "BAIXO RISCO"
    }
  },
  "emails": [
    {
      "email": "etar07@gmail.com",
      "qualidade": "BOM",
      "prioridade": "ALTA"
    }
  ],
  "telefones": [
    {
      "telefone": "11999887766",
      "tipo": "TELEFONE MÓVEL",
      "whatsapp": "SIM"
    }
  ]
}
```

**Becomes**:

```sql
-- 1. Entity
INSERT INTO core.entities (national_id, name, canonical_name, ...)
VALUES ('78706009891', 'EDSON TARRAF', 'EDSON TARRAF', ...);

-- 2. Profile
INSERT INTO core.entity_profiles (sex, birth_date, metadata, ...)
VALUES ('M', '1958-01-28', '{"mother_name":"OLINDA ADDAS TARRAF"}', ...);

-- 3. Financials
INSERT INTO core.entity_financials (reported_income, credit_score, risk_score, ...)
VALUES (39368.00, 750, 0.3, ...);  -- Note: 20720 * 1.9 = 39368

-- 4. Email
INSERT INTO core.entity_emails (email, is_primary, is_verified, ...)
VALUES ('etar07@gmail.com', true, true, ...);

-- 5. Phone
INSERT INTO core.entity_phones (phone, is_primary, is_whatsapp, ...)
VALUES ('11999887766', true, true, ...);
```

## Related Files

- Implementation: `src/db_storage.rs`
- SQL Reference: `docs/queries/work_api_enrichment.sql`
- Rate Limiting: `docs/integrations/WORK_API_RATE_LIMITING.md`
- Batch Import: `examples/import_json_to_db.rs`
