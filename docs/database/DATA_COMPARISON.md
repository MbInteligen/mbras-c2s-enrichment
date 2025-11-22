# Data Comparison: Database vs Work API

## CPF: 11089118899 - Rogerio de Campos Morais

### Current Database Data (BEFORE Enrichment)

| Field | Current DB Value | Work API Value | Status |
|-------|-----------------|----------------|--------|
| **Name** | Rogerio de Campos Morais | ROGERIO DE CAMPOS MORAIS | ✅ SAME (case difference) |
| **CPF** | 11089118899 | 11089118899 | ✅ SAME |
| **Sex** | M | M - MASCULINO | ✅ SAME |
| **Birth Date** | 1969-04-01 | 01/04/1969 | ✅ SAME |
| **Mother Name** | MARILIA SAMPAIO DE CAMPOS MORAIS | MARILIA SAMPAIO DE CAMPOS MORAIS | ✅ SAME |
| **Father Name** | null | SEM INFORMAÇÃO | ⚠️ CAN UPDATE |
| **Marital Status** | null | "" (empty) | ⚠️ EMPTY IN API |
| **Education** | null | ENSINO SUPERIOR COMPLETO | ✅ CAN ADD |
| **Nationality** | null | BRASILEIRA | ✅ CAN ADD |
| **Is Enriched** | true | - | ✅ ALREADY MARKED |
| **Enriched At** | 2024-07-07 | - | 📅 OLD (7 months ago) |

### Financial Data (MISSING in DB)

| Field | Current DB | Work API | Action |
|-------|-----------|----------|--------|
| **Income** | ❌ NONE | 28623.87 | ✅ **ADD** (54,385.35 after 1.9x) |
| **Credit Score** | ❌ NONE | 968 | ✅ **ADD** |
| **Risk Level** | ❌ NONE | BAIXISSIMO RISCO | ✅ **ADD** |

### Emails Comparison

| Email | In DB? | In Work API? | Action |
|-------|--------|--------------|--------|
| campos.morais@uol.com.br | ✅ YES | ✅ YES | ✅ KEEP |
| rmorais@crossbeam.com | ✅ YES | ✅ YES | ✅ KEEP |
| rmorais@interare.com.br | ✅ YES | ✅ YES | ✅ KEEP |
| rogermorais@hotmail.com | ✅ YES | ✅ YES | ✅ KEEP |
| campos.morais@icloud.com | ❌ NO | ✅ YES | ✅ **ADD NEW** |

**Summary:** 4 existing emails match, 1 new email to add

### Phones Comparison

| Phone | In DB? | In Work API? | Action |
|-------|--------|--------------|--------|
| 55054244 | ✅ YES | ✅ YES (1155054244) | ✅ KEEP |
| 991737692 | ✅ YES | ✅ YES (11991737692) | ✅ KEEP |
| 35682100 | ✅ YES | ✅ YES (1135682100) | ✅ KEEP |
| 40621515 | ✅ YES | ✅ YES (1140621515) | ✅ KEEP |
| 991845880 | ✅ YES | ✅ YES (11991845880) | ✅ KEEP |
| 37434068 | ✅ YES | ✅ YES (1137434068) | ✅ KEEP |
| 991657096 | ✅ YES | ✅ YES (11991657096) | ✅ KEEP |
| - | ❌ NO | ✅ YES (1137465693) | ✅ **ADD NEW** |
| - | ❌ NO | ✅ YES (1160186458) | ✅ **ADD NEW** |
| - | ❌ NO | ✅ YES (05622001) | ✅ **ADD NEW** |
| - | ❌ NO | ✅ YES (11988585805) | ✅ **ADD NEW** |
| - | ❌ NO | ✅ YES (11988590755) | ✅ **ADD NEW** |
| - | ❌ NO | ✅ YES (11991652900) | ✅ **ADD NEW** |

**Summary:** 7 existing phones match, 6 new phones to add

---

## Other Enriched CPFs Status

### CPF: 15711178814 - Maria Teresa Pedro Vieira Elias
**Status:** ❌ NOT in database - **NEEDS FULL INSERT**

### CPF: 16060916899 - (From phone lookup)
**Status:** ❌ NOT in database - **NEEDS FULL INSERT**

---

## Implementation Strategy

### For EXISTING entities (like 11089118899):
```sql
UPDATE core.entity_profiles SET
  education_level = COALESCE(education_level, $1),  -- Only update if NULL
  nationality = COALESCE(nationality, $2),
  metadata = metadata || $3,  -- Merge metadata
  updated_at = now()
WHERE entity_id = $4;

INSERT INTO core.entity_financials (...)
VALUES (...)
ON CONFLICT (entity_id, financial_year) DO UPDATE SET ...;

INSERT INTO core.entity_emails (entity_id, email, ...)
VALUES (...)
ON CONFLICT (email) DO NOTHING;  -- Keep existing, add new

INSERT INTO core.entity_phones (entity_id, phone, ...)
VALUES (...)
ON CONFLICT (phone) DO NOTHING;  -- Keep existing, add new
```

### For NEW entities (like 15711178814, 16060916899):
```sql
INSERT INTO core.entities (...) VALUES (...);
INSERT INTO core.entity_profiles (...) VALUES (...);
INSERT INTO core.entity_financials (...) VALUES (...);
INSERT INTO core.entity_emails (...) VALUES (...);
INSERT INTO core.entity_phones (...) VALUES (...);
```

---

## Update Logic

### Rule 1: Never Overwrite Existing Data
- If DB has a value and API has a value → KEEP DB value
- If DB is NULL and API has value → UPDATE with API value
- If both are NULL/empty → Keep NULL

### Rule 2: Always Add New Contact Info
- New emails → INSERT (ignore conflicts)
- New phones → INSERT (ignore conflicts)

### Rule 3: Always Update Financial Data
- Financial data changes over time → UPDATE to latest

### Rule 4: Update Enrichment Timestamp
- Set `enriched_at = now()` on every enrichment
- Set `is_enriched = true`

---

## Fields to Update (Only if NULL)

For **entity_profiles**:
- ✅ `education_level` - if NULL
- ✅ `nationality` - if NULL
- ✅ `marital_status` - if NULL and API has value
- ✅ `occupation` - if NULL
- ✅ `metadata.father_name` - if NULL
- ✅ `metadata.cor` - if NULL
- ✅ `metadata.municipioNascimento` - if NULL
- ✅ `metadata.cns` - if NULL

For **entity_financials**:
- ✅ ALWAYS update (data changes yearly)

For **entity_emails/phones**:
- ✅ ALWAYS add new ones
- ✅ NEVER remove existing ones

---

## Next Steps

1. ✅ Data comparison complete
2. 🔄 Implement database storage service
3. 🔄 Add enrichment after C2S flow
4. 🔄 Test with existing CPF (11089118899)
5. 🔄 Test with new CPFs (15711178814, 16060916899)
