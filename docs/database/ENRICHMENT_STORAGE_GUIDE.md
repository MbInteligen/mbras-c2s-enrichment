# Enrichment Storage Guide - Rust C2S API

**Version:** 1.0  
**Last Updated:** 2025-11-22  
**Status:** Production-Ready  
**Source File:** `src/db_storage.rs`

---

## Table of Contents

1. [Overview](#overview)
2. [Database Schema](#database-schema)
3. [Data Flow](#data-flow)
4. [SQL Queries](#sql-queries)
5. [Data Mapping](#data-mapping)
6. [Special Cases](#special-cases)
7. [Code Examples](#code-examples)
8. [Verification Queries](#verification-queries)

---

## 1. Overview

### What is Enrichment?

**Enrichment** is the process of taking basic lead/customer data (name, phone, email) and augmenting it with comprehensive demographic, financial, and behavioral information from external data providers (primarily Work API).

### Why We Store Enrichment Data

1. **Persistence** - Work API data is expensive; we store it to avoid re-querying
2. **Analytics** - Enable BI dashboards and lead scoring
3. **Lead Management** - Track enrichment status and quality
4. **Audit Trail** - Maintain historical snapshots of enriched data
5. **C2S Integration** - Send enriched data to Contact2Sale CRM

### High-Level Flow

```
Work API Response → Data Extraction → Transformation → Database Storage
                                                              ↓
                                                    4 Core Tables:
                                                    - core.parties
                                                    - core.people
                                                    - core.party_contacts
                                                    - core.party_enrichments
```

---

## 2. Database Schema

### Architecture Overview

The database uses a **Party Model** architecture:

```
core.parties (golden record)
    ├── core.people (PF specialization - 1:1)
    ├── core.companies (PJ specialization - 1:1)
    ├── core.party_contacts (N:M - emails, phones, whatsapp)
    └── core.party_enrichments (1:1 - raw API snapshots)
```

### Core Tables Used in Enrichment

#### `core.parties` - Golden Record (Identity Hub)

Primary table for all party types with common identity attributes.

| Column | Type | Constraints | Purpose | Populated By |
|--------|------|------------|---------|--------------|
| `id` | UUID | PK | Party identifier | `gen_random_uuid()` |
| `party_type` | TEXT | NOT NULL | Discriminator: 'person' or 'company' | Always 'person' for enrichment |
| `cpf_cnpj` | TEXT | - | National ID (CPF/CNPJ) | Work API: `DadosBasicos.cpf` |
| `full_name` | TEXT | NOT NULL | Display name | Work API: `DadosBasicos.nome` |
| `normalized_name` | TEXT | - | Uppercase for matching | `UPPER(full_name)` |
| `enriched` | BOOLEAN | DEFAULT false | Enrichment flag | Set to `true` after enrichment |
| `birth_date` | DATE | - | Birth date | Work API: `DadosBasicos.dataNascimento` |
| `sex` | TEXT | - | Gender (M/F) | Work API: `DadosBasicos.sexo` (first char) |
| `mother_name` | TEXT | - | Mother's name | Work API: `DadosBasicos.nomeMae` |
| `created_at` | TIMESTAMPTZ | DEFAULT now() | Creation timestamp | Auto |
| `updated_at` | TIMESTAMPTZ | DEFAULT now() | Last update | Auto |

**Indexes:**
- `idx_parties_cpf_cnpj` ON (cpf_cnpj) WHERE cpf_cnpj IS NOT NULL
- `idx_parties_normalized_name` ON (normalized_name)
- `idx_parties_party_type` ON (party_type)

**Key Behavior:**
- **NO unique constraint** on `cpf_cnpj` - allows historical duplicates
- **Deduplication** via `SELECT ... WHERE cpf_cnpj = ? LIMIT 1` before insert
- **Upsert logic** - UPDATE if exists, INSERT if not

---

#### `core.people` - Person Specialization (PF)

Person-specific attributes, one-to-one with parties where `party_type = 'person'`.

| Column | Type | Constraints | Purpose | Populated By |
|--------|------|------------|---------|--------------|
| `party_id` | UUID | PK, FK(parties.id) | Link to party | Same as `core.parties.id` |
| `full_name` | TEXT | - | Person's full name | Work API: `DadosBasicos.nome` |
| `mothers_name` | TEXT | - | Mother's name | Work API: `DadosBasicos.nomeMae` |
| `birth_date` | DATE | - | Date of birth | Work API: `DadosBasicos.dataNascimento` |
| `sex` | TEXT | - | Gender | Work API: `DadosBasicos.sexo` (first char) |
| `marital_status` | TEXT | - | Marital status | Work API: `DadosBasicos.estadoCivil` |
| `document_cpf` | TEXT | - | CPF document | Work API: `DadosBasicos.cpf` |
| `created_at` | TIMESTAMPTZ | DEFAULT now() | Creation timestamp | Auto |
| `updated_at` | TIMESTAMPTZ | DEFAULT now() | Last update | Auto |

**Constraints:**
- **FK CASCADE** on `party_id` - deleting party deletes person record

**Upsert Behavior:**
```sql
ON CONFLICT (party_id) DO UPDATE
SET full_name = EXCLUDED.full_name,
    mothers_name = COALESCE(EXCLUDED.mothers_name, core.people.mothers_name),
    birth_date = COALESCE(EXCLUDED.birth_date, core.people.birth_date),
    ...
```

**Note:** Uses `COALESCE` to preserve existing values if new data is NULL.

---

#### `core.party_contacts` - Unified Contacts

All contact methods (email, phone, whatsapp) in a single table with type discrimination.

| Column | Type | Constraints | Purpose | Populated By |
|--------|------|------------|---------|--------------|
| `contact_id` | UUID | PK | Contact identifier | `gen_random_uuid()` |
| `party_id` | UUID | FK(parties.id), NOT NULL | Party link | From parties insert |
| `contact_type` | ENUM | NOT NULL | 'email', 'phone', 'whatsapp' | Based on Work API source |
| `value` | TEXT | NOT NULL | Contact value | Email or phone number |
| `is_primary` | BOOLEAN | DEFAULT false | Primary contact flag | `true` for first in array |
| `is_verified` | BOOLEAN | DEFAULT false | Verification status | Email: `qualidade == "BOM"` |
| `is_whatsapp` | BOOLEAN | DEFAULT false | WhatsApp capability | Phone: `whatsapp == "SIM"` |
| `source` | TEXT | - | Data source | Work API metadata |
| `confidence` | NUMERIC(3,2) | CHECK [0,1] | Quality score (0-1) | Mapped from Work API |
| `valid_from` | TIMESTAMPTZ | DEFAULT now() | Validity start | Auto |
| `valid_to` | TIMESTAMPTZ | - | Validity end | NULL (currently active) |
| `created_at` | TIMESTAMPTZ | DEFAULT now() | Creation timestamp | Auto |
| `updated_at` | TIMESTAMPTZ | DEFAULT now() | Last update | Auto |

**Constraints:**
- `uq_party_contact_unique` UNIQUE (party_id, contact_type, value)

**Indexes:**
- `idx_party_contacts_party` ON (party_id)
- `idx_party_contacts_value` ON (value)

**Deduplication:**
- Constraint enforces uniqueness per party
- Duplicate inserts are silently ignored via `ON CONFLICT DO NOTHING`

**Normalization:**
- **Emails**: Stored as `LOWER(email)` (lowercase)
- **Phones**: Stored as digits only via `phone.chars().filter(|c| c.is_ascii_digit()).collect()`

---

#### `core.party_enrichments` - Enrichment Snapshots

Stores raw API responses and quality metrics for each enrichment event.

| Column | Type | Constraints | Purpose | Populated By |
|--------|------|------------|---------|--------------|
| `enrichment_id` | UUID | PK | Enrichment identifier | `gen_random_uuid()` |
| `party_id` | UUID | FK(parties.id), UNIQUE | Party link (1:1) | From parties insert |
| `provider` | TEXT | - | Data provider | Always 'work_api' |
| `raw_payload` | JSONB | - | Full API response | Entire Work API JSON |
| `normalized_data` | JSONB | DEFAULT '{}' | Processed data | Currently empty |
| `quality_score` | NUMERIC(3,2) | - | Overall quality (0-1) | Derived from risk_score |
| `enriched_at` | TIMESTAMPTZ | - | Enrichment timestamp | `now()` |
| `created_at` | TIMESTAMPTZ | DEFAULT now() | Creation timestamp | Auto |

**Upsert Behavior:**
```sql
ON CONFLICT (party_id) DO UPDATE
SET provider = EXCLUDED.provider,
    raw_payload = EXCLUDED.raw_payload,
    quality_score = GREATEST(core.party_enrichments.quality_score, EXCLUDED.quality_score),
    enriched_at = EXCLUDED.enriched_at
```

**Note:** Keeps the **highest** quality_score if re-enriched.

---

### Supporting Tables (Not Directly Modified by Enrichment)

These tables exist in the schema but are **not currently populated** by the enrichment flow:

- `core.companies` - PJ specialization (future use for CNPJ enrichment)
- `core.real_estate_properties` - Property ownership (future enhancement)
- `core.ownerships` - Property-party relationships (future)
- `core.party_relationships` - Family/business relationships (future)

---

## 3. Data Flow

### Step-by-Step Enrichment Process

```
┌─────────────────────────────────────────────────────────────────┐
│ 1. API CALL: Work API Returns Complete Response                │
│    URL: https://api.workrb.com.br/data/completa?chave=XXX&cpf=YYY │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│ 2. DATA EXTRACTION (src/db_storage.rs:42-120)                  │
│    - Extract DadosBasicos (name, birth_date, sex, mother_name) │
│    - Extract DadosEconomicos (income, credit_score, risk_level)│
│    - Extract emails array                                       │
│    - Extract telefones array                                    │
│    - Build metadata JSONB objects                               │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│ 3. TRANSFORMATION                                               │
│    - Dates: "DD/MM/YYYY" → "YYYY-MM-DD"                        │
│    - Sex: "M - MASCULINO" → "M"                                │
│    - Name: Store as-is + uppercase canonical_name              │
│    - Emails: Lowercase                                          │
│    - Phones: Digits only                                        │
│    - Risk level: Text → Numeric score (0.1-0.9)                │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│ 4. DATABASE STORAGE (Sequential Order)                         │
│                                                                 │
│    Step 4.1: Upsert core.parties                               │
│              - Check if CPF exists                             │
│              - UPDATE if exists, INSERT if not                 │
│              - Get party_id                                    │
│                                                                 │
│    Step 4.2: Upsert core.people                                │
│              - Use party_id from Step 4.1                      │
│              - ON CONFLICT (party_id) DO UPDATE                │
│                                                                 │
│    Step 4.3: Store core.party_contacts (emails)                │
│              - Loop through emails array                        │
│              - INSERT each email                                │
│              - ON CONFLICT DO NOTHING (ignore duplicates)       │
│                                                                 │
│    Step 4.4: Store core.party_contacts (phones)                │
│              - Loop through telefones array                     │
│              - INSERT each phone/whatsapp                       │
│              - ON CONFLICT DO NOTHING (ignore duplicates)       │
│                                                                 │
│    Step 4.5: Store core.party_enrichments                      │
│              - Store raw Work API JSON payload                  │
│              - ON CONFLICT (party_id) DO UPDATE                │
│              - Keep highest quality_score                       │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│ 5. SUCCESS: Return party_id                                    │
└─────────────────────────────────────────────────────────────────┘
```

### Dependency Order

**Critical:** Tables must be updated in this order due to foreign key constraints:

1. **core.parties** (no dependencies)
2. **core.people** (depends on parties.id)
3. **core.party_contacts** (depends on parties.id)
4. **core.party_enrichments** (depends on parties.id)

### Transaction Handling

**Current Implementation:** No explicit transactions (auto-commit per query)

**Behavior:**
- If Step 4.3 fails, Steps 4.1 and 4.2 are already committed
- Partial enrichment is possible (e.g., party + person, but no contacts)
- No rollback on failure

**Future Enhancement:** Wrap entire flow in a transaction for atomicity.

---

## 4. SQL Queries

### 4.1 Party Management (core.parties)

#### Check if Party Exists

```sql
SELECT id 
FROM core.parties 
WHERE cpf_cnpj = $1 
LIMIT 1;
```

**Parameters:**
- `$1` - CPF (11 digits, no formatting)

**Returns:**
- `(Uuid,)` if exists
- `None` if not found

---

#### Update Existing Party

```sql
UPDATE core.parties
SET party_type = COALESCE(party_type, $2),
    full_name = COALESCE(full_name, $3),
    normalized_name = COALESCE(normalized_name, $4),
    enriched = true,
    birth_date = COALESCE(birth_date, $5),
    sex = COALESCE(sex, $6),
    mother_name = COALESCE(mother_name, $7),
    opening_date = COALESCE(opening_date, $8),
    company_type = COALESCE(company_type, $9),
    company_size = COALESCE(company_size, $10),
    updated_at = now()
WHERE id = $1
```

**Parameters:**
1. `$1` (UUID) - party_id
2. `$2` (TEXT) - "person"
3. `$3` (TEXT) - nome (full name)
4. `$4` (TEXT) - canonical_name (uppercase)
5. `$5` (DATE) - birth_date (converted from DD/MM/YYYY)
6. `$6` (TEXT) - sex ("M" or "F")
7. `$7` (TEXT) - mother_name
8. `$8` (DATE) - NULL (for PJ only)
9. `$9` (TEXT) - NULL (for PJ only)
10. `$10` (TEXT) - NULL (for PJ only)

**Logic:** Uses `COALESCE` to preserve existing values if new value is NULL.

---

#### Insert New Party

```sql
INSERT INTO core.parties (
    id, party_type, cpf_cnpj, full_name, normalized_name, enriched,
    birth_date, sex, mother_name, opening_date, company_type, company_size,
    created_at, updated_at
)
VALUES (
    gen_random_uuid(), $1, $2, $3, $4, true, 
    $5, $6, $7, $8, $9, $10, 
    now(), now()
)
RETURNING id
```

**Parameters:** Same as Update query (minus $1 party_id)

**Returns:** `(Uuid,)` - newly created party_id

---

### 4.2 Person Specialization (core.people)

#### Upsert Person

```sql
INSERT INTO core.people (
    party_id, full_name, mothers_name, birth_date, sex,
    marital_status, document_cpf, created_at, updated_at
)
VALUES ($1, $2, $3, $4, $5, $6, $7, now(), now())
ON CONFLICT (party_id) DO UPDATE
SET full_name = EXCLUDED.full_name,
    mothers_name = COALESCE(EXCLUDED.mothers_name, core.people.mothers_name),
    birth_date = COALESCE(EXCLUDED.birth_date, core.people.birth_date),
    sex = COALESCE(EXCLUDED.sex, core.people.sex),
    marital_status = COALESCE(EXCLUDED.marital_status, core.people.marital_status),
    document_cpf = COALESCE(EXCLUDED.document_cpf, core.people.document_cpf),
    updated_at = EXCLUDED.updated_at
```

**Parameters:**
1. `$1` (UUID) - party_id
2. `$2` (TEXT) - nome
3. `$3` (TEXT) - nomeMae
4. `$4` (DATE) - dataNascimento (converted)
5. `$5` (TEXT) - sexo (first char)
6. `$6` (TEXT) - estadoCivil
7. `$7` (TEXT) - cpf

**Conflict Resolution:** Updates all fields, but preserves existing values via `COALESCE` for optional fields.

---

### 4.3 Contacts - Emails (core.party_contacts)

#### Insert Email Contact

```sql
INSERT INTO core.party_contacts (
    contact_id, party_id, contact_type, value,
    is_primary, is_verified, is_whatsapp, source,
    confidence, valid_from, valid_to, created_at, updated_at
)
VALUES (
    gen_random_uuid(), $1, 'email'::core.contact_type_enum, $2, 
    $3, $4, false, $5,
    $6, now(), NULL, now(), now()
)
ON CONFLICT ON CONSTRAINT uq_party_contact_unique DO NOTHING
```

**Parameters:**
1. `$1` (UUID) - party_id
2. `$2` (TEXT) - email (lowercase normalized)
3. `$3` (BOOLEAN) - is_primary (true if idx == 0)
4. `$4` (BOOLEAN) - is_verified (true if qualidade == "BOM")
5. `$5` (TEXT) - source (e.g., "prioridade": "ALTA")
6. `$6` (NUMERIC) - confidence (parsed from qualidade, can be NULL)

**Deduplication:** `ON CONFLICT DO NOTHING` - silently ignores duplicate emails for same party.

**Primary Selection Logic:**
```rust
let is_primary = idx == 0; // First email in Work API array
```

---

### 4.4 Contacts - Phones (core.party_contacts)

#### Insert Phone Contact

```sql
INSERT INTO core.party_contacts (
    contact_id, party_id, contact_type, value,
    is_primary, is_verified, is_whatsapp, source,
    confidence, valid_from, valid_to, created_at, updated_at
)
VALUES (
    gen_random_uuid(), $1,
    CASE WHEN $3 THEN 'whatsapp'::core.contact_type_enum 
         ELSE 'phone'::core.contact_type_enum END,
    $2, $4, true, $3, $5, $6, now(), NULL, now(), now()
)
ON CONFLICT ON CONSTRAINT uq_party_contact_unique DO NOTHING
```

**Parameters:**
1. `$1` (UUID) - party_id
2. `$2` (TEXT) - phone (digits only, normalized)
3. `$3` (BOOLEAN) - is_whatsapp (true if whatsapp == "SIM")
4. `$4` (BOOLEAN) - is_primary (true if idx == 0)
5. `$5` (TEXT) - source (e.g., operadora: "VIVO")
6. `$6` (NUMERIC) - confidence (parsed from status, can be NULL)

**Phone Normalization:**
```rust
let normalized: String = phone.chars()
    .filter(|c| c.is_ascii_digit())
    .collect();
```

**Example:**
- Input: "(11) 99887-7665"
- Stored: "11998877665"

---

### 4.5 Enrichment Snapshot (core.party_enrichments)

#### Insert/Update Enrichment

```sql
INSERT INTO core.party_enrichments (
    enrichment_id, party_id, provider, raw_payload, normalized_data,
    quality_score, enriched_at, created_at
)
VALUES (gen_random_uuid(), $1, 'work_api', $2, '{}'::jsonb, $3, now(), now())
ON CONFLICT (party_id) DO UPDATE
SET provider = EXCLUDED.provider,
    raw_payload = EXCLUDED.raw_payload,
    quality_score = GREATEST(core.party_enrichments.quality_score, EXCLUDED.quality_score),
    enriched_at = EXCLUDED.enriched_at
```

**Parameters:**
1. `$1` (UUID) - party_id
2. `$2` (JSONB) - raw_payload (entire Work API JSON response)
3. `$3` (NUMERIC) - quality_score (0-1 scale, derived from risk_score)

**Quality Score Calculation:**
```rust
let quality_score = risk_score
    .as_ref()
    .and_then(|bd| bd.to_string().parse::<f64>().ok())
    .unwrap_or(0.5);
```

**Conflict Resolution:** 
- Updates raw_payload with latest data
- Keeps **highest** quality_score via `GREATEST()` function

---

## 5. Data Mapping

### Work API Fields → Database Columns

#### DadosBasicos (Personal Information)

| Work API Field | Database Table | Database Column | Transformation |
|----------------|----------------|-----------------|----------------|
| `DadosBasicos.cpf` | core.parties | cpf_cnpj | Direct (11 digits) |
| `DadosBasicos.nome` | core.parties | full_name | Direct |
| `DadosBasicos.nome` | core.parties | normalized_name | `UPPER(nome)` |
| `DadosBasicos.nome` | core.people | full_name | Direct |
| `DadosBasicos.dataNascimento` | core.parties | birth_date | "29/04/1975" → "1975-04-29" |
| `DadosBasicos.dataNascimento` | core.people | birth_date | Same conversion |
| `DadosBasicos.sexo` | core.parties | sex | "M - MASCULINO" → "M" |
| `DadosBasicos.sexo` | core.people | sex | Same (first char) |
| `DadosBasicos.nomeMae` | core.parties | mother_name | Direct |
| `DadosBasicos.nomeMae` | core.people | mothers_name | Direct |
| `DadosBasicos.estadoCivil` | core.people | marital_status | Direct |
| `DadosBasicos.cpf` | core.people | document_cpf | Direct |

#### DadosEconomicos (Financial Data)

| Work API Field | Database Table | Database Column | Transformation |
|----------------|----------------|-----------------|----------------|
| `DadosEconomicos.score.scoreCSBAFaixaRisco` | core.party_enrichments | quality_score | Mapped to 0.1-0.9 (see below) |

**Risk Level Mapping:**

| Work API Value | quality_score |
|----------------|---------------|
| "BAIXISSIMO RISCO" | 0.1 |
| "BAIXO RISCO" | 0.3 |
| "MEDIO RISCO" | 0.5 |
| "ALTO RISCO" | 0.7 |
| "ALTISSIMO RISCO" | 0.9 |

#### emails Array

| Work API Field | Database Table | Database Column | Transformation |
|----------------|----------------|-----------------|----------------|
| `emails[].email` | core.party_contacts | value | `LOWER(email)` |
| `emails[].qualidade` | core.party_contacts | is_verified | `qualidade == "BOM"` → true |
| `emails[].prioridade` | core.party_contacts | source | Direct (metadata) |
| `emails[].qualidade` | core.party_contacts | confidence | Parse as float (0-1) |
| Position in array (idx == 0) | core.party_contacts | is_primary | First email → true |
| - | core.party_contacts | contact_type | Always 'email' |

#### telefones Array

| Work API Field | Database Table | Database Column | Transformation |
|----------------|----------------|-----------------|----------------|
| `telefones[].telefone` | core.party_contacts | value | Digits only |
| `telefones[].whatsapp` | core.party_contacts | is_whatsapp | `whatsapp == "SIM"` → true |
| `telefones[].whatsapp` | core.party_contacts | contact_type | WhatsApp → 'whatsapp', else 'phone' |
| `telefones[].operadora` | core.party_contacts | source | Direct (metadata) |
| `telefones[].status` | core.party_contacts | confidence | Parse as float (0-1) |
| Position in array (idx == 0) | core.party_contacts | is_primary | First phone → true |

#### Full API Response

| Work API Field | Database Table | Database Column | Transformation |
|----------------|----------------|-----------------|----------------|
| Entire JSON response | core.party_enrichments | raw_payload | Direct (JSONB) |

---

### Metadata Storage (JSONB Fields)

Work API fields not mapped to dedicated columns are stored in JSONB `metadata`:

**core.parties.metadata** (not currently implemented, but planned):
- `cor` - Ethnicity/race
- `municipioNascimento` - Birth municipality
- `cns` - National health card number
- `c2s_lead_id` - Contact2Sale lead tracking (if enrichment triggered from C2S)

**core.party_contacts.metadata** (email):
- `prioridade` - Email priority level
- `qualidade` - Email quality rating
- `email_pessoal` - Personal email flag
- `blacklist` - Blacklist status

**core.party_contacts.metadata** (phone):
- `status` - Phone status indicator

---

## 6. Special Cases

### 6.1 Contact Deduplication

**Problem:** Work API may return duplicate contacts, or re-enrichment may attempt to insert existing contacts.

**Solution:** Database constraint enforces uniqueness:

```sql
CONSTRAINT uq_party_contact_unique UNIQUE (party_id, contact_type, value)
```

**Behavior:**
```sql
ON CONFLICT ON CONSTRAINT uq_party_contact_unique DO NOTHING
```

**Result:** Duplicate inserts are **silently ignored** (no error thrown).

---

### 6.2 Email Normalization

**Problem:** Work API returns emails with varying case: `JoAo@Gmail.com`, `joao@gmail.com`

**Solution:** Store all emails as lowercase:

```rust
.bind(email_addr.to_lowercase())
```

**Deduplication:** `JOAO@GMAIL.COM` and `joao@gmail.com` are treated as same contact.

---

### 6.3 Phone Normalization

**Problem:** Work API returns phones with formatting: `(11) 99887-7665`, `11998877665`, `+55 11 99887-7665`

**Solution:** Strip all non-numeric characters:

```rust
let normalized: String = phone.chars()
    .filter(|c| c.is_ascii_digit())
    .collect();
```

**Stored Value:** `11998877665` (digits only)

**Benefits:**
- Consistent format for deduplication
- Easy to query/match
- Can reconstruct formatted display as needed

---

### 6.4 Primary Contact Selection

**Rule:** First contact in Work API array is marked as primary.

**Implementation:**

```rust
for (idx, email_obj) in emails.iter().enumerate() {
    let is_primary = idx == 0; // First email is primary
    // ...
}
```

**Result:**
- `emails[0]` → `is_primary = true`
- `emails[1+]` → `is_primary = false`

**Note:** Only one primary contact per type per party.

---

### 6.5 Date Format Conversion

**Problem:** Work API returns dates as `"DD/MM/YYYY"`, PostgreSQL expects `"YYYY-MM-DD"`.

**Solution:** Parse and reformat:

```rust
fn parse_br_date(date_str: &str) -> Result<chrono::NaiveDate, chrono::ParseError> {
    chrono::NaiveDate::parse_from_str(date_str, "%d/%m/%Y")
}
```

**Example:**
- Input: `"29/04/1975"`
- Parsed: `NaiveDate { year: 1975, month: 4, day: 29 }`
- Stored: `"1975-04-29"`

---

### 6.6 Sex/Gender Extraction

**Problem:** Work API returns verbose format: `"M - MASCULINO"` or `"F - FEMININO"`

**Solution:** Extract first character only:

```rust
let sexo = dados_basicos
    .and_then(|d| d.get("sexo"))
    .and_then(|v| v.as_str())
    .and_then(|s| s.chars().next())  // First char
    .unwrap_or('M');
```

**Stored Values:**
- `"M"` (not "M - MASCULINO")
- `"F"` (not "F - FEMININO")

---

### 6.7 Lead Tracking (C2S Integration)

**Feature:** Track which enrichments originated from Contact2Sale leads.

**Implementation:**

```rust
pub async fn store_enriched_person_with_lead(
    &self,
    cpf: &str,
    work_data: &WorkApiCompleteResponse,
    lead_id: Option<&str>,  // ← C2S lead_id
) -> Result<Uuid, AppError>
```

**Storage:**
- Lead ID is **injected** into raw_payload before storing:

```rust
let mut enrichment_payload = work_data.clone();
if let Some(lid) = lead_id {
    enrichment_payload["lead_id"] = json!(lid);
}
```

**Query by Lead:**

```sql
SELECT party_id 
FROM core.party_enrichments 
WHERE raw_payload->>'lead_id' = 'bf1a88eaa4ab34b01a257536563fb42b';
```

**Metadata Structure:**

```json
{
  "lead_id": "bf1a88eaa4ab34b01a257536563fb42b",
  "c2s_source": "api_enrichment",
  "enriched_at": "2025-11-22T..."
}
```

---

### 6.8 Partial Enrichment Handling

**Scenario:** Work API returns incomplete data (missing emails, phones, etc.)

**Behavior:**
- **Required fields** (cpf, nome): Must be present or insert fails
- **Optional fields** (birth_date, mother_name, etc.): Use `Option<T>` types
- **Missing arrays** (emails, telefones): Skip iteration if array is missing

**Implementation:**

```rust
if let Some(emails) = work_data.get("emails").and_then(|e| e.as_array()) {
    self.store_party_emails(party_id, emails).await?;
}
// If no emails, this block is skipped (no error)
```

**Result:** Party is enriched with available data, missing fields remain NULL.

---

### 6.9 Re-Enrichment (Updates)

**Scenario:** Same CPF is enriched multiple times.

**Behavior:**

1. **core.parties**: Updates existing record, preserves existing values via `COALESCE`
2. **core.people**: Updates existing record, same `COALESCE` logic
3. **core.party_contacts**: New contacts are added, duplicates ignored
4. **core.party_enrichments**: Updates raw_payload, **keeps highest quality_score**

**Quality Score Logic:**

```sql
quality_score = GREATEST(
    core.party_enrichments.quality_score,  -- Old value
    EXCLUDED.quality_score                  -- New value
)
```

**Example:**
- First enrichment: quality_score = 0.5
- Second enrichment: quality_score = 0.7
- Stored: 0.7 (highest)

**Rationale:** Assumes enrichment quality improves over time or with better data sources.

---

### 6.10 Error Handling

**Current Implementation:** Minimal error handling (fail fast)

**Behavior:**

1. **Database errors** (connection, query errors) → Return `AppError::DatabaseError`
2. **Parsing errors** (invalid JSON, date format) → Use `Option` types, treat as NULL
3. **Missing data** → Use `.unwrap_or()` with safe defaults

**Example:**

```rust
let sexo = dados_basicos
    .and_then(|d| d.get("sexo"))
    .and_then(|v| v.as_str())
    .and_then(|s| s.chars().next())
    .unwrap_or('M');  // Default to 'M' if missing
```

**Future Enhancement:** Add comprehensive error logging and retry logic.

---

## 7. Code Examples

### 7.1 Main Storage Function

**Function:** `store_enriched_person_with_lead()`

**Location:** `src/db_storage.rs:35-305`

**Signature:**

```rust
pub async fn store_enriched_person_with_lead(
    &self,
    cpf: &str,
    work_data: &WorkApiCompleteResponse,
    lead_id: Option<&str>,
) -> Result<Uuid, AppError>
```

**Flow:**

```rust
// Step 1: Extract data from Work API response
let dados_basicos = work_data.get("DadosBasicos");
let dados_econ = work_data.get("DadosEconomicos");

let nome = dados_basicos
    .and_then(|d| d.get("nome"))
    .and_then(|v| v.as_str())
    .unwrap_or("");

let sexo = dados_basicos
    .and_then(|d| d.get("sexo"))
    .and_then(|v| v.as_str())
    .and_then(|s| s.chars().next())
    .unwrap_or('M');

let data_nasc = dados_basicos
    .and_then(|d| d.get("dataNascimento"))
    .and_then(|v| v.as_str())
    .and_then(|d| parse_br_date(d).ok());

// Step 2: Canonical name
let canonical_name = nome.to_uppercase();

// Step 3: Risk score mapping
let risk_score = risk_level.and_then(|r| match r {
    "BAIXISSIMO RISCO" => BigDecimal::from_str("0.1").ok(),
    "BAIXO RISCO" => BigDecimal::from_str("0.3").ok(),
    "MEDIO RISCO" => BigDecimal::from_str("0.5").ok(),
    "ALTO RISCO" => BigDecimal::from_str("0.7").ok(),
    "ALTISSIMO RISCO" => BigDecimal::from_str("0.9").ok(),
    _ => None,
});

// Step 4: Upsert party
let party_id = match sqlx::query_as::<_, (Uuid,)>(
    "SELECT id FROM core.parties WHERE cpf_cnpj = $1 LIMIT 1",
)
.bind(cpf)
.fetch_optional(&self.pool)
.await
.map_err(AppError::DatabaseError)?
{
    Some(existing) => {
        // Update existing
        sqlx::query("UPDATE core.parties SET ... WHERE id = $1")
            .bind(existing.0)
            // ... bind parameters
            .execute(&self.pool)
            .await
            .map_err(AppError::DatabaseError)?;
        existing.0
    }
    None => {
        // Insert new
        let inserted: (Uuid,) = sqlx::query_as(
            "INSERT INTO core.parties (...) VALUES (...) RETURNING id",
        )
        // ... bind parameters
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::DatabaseError)?;
        inserted.0
    }
};

// Step 5: Upsert people
sqlx::query("INSERT INTO core.people (...) VALUES (...) ON CONFLICT (party_id) DO UPDATE ...")
    .bind(party_id)
    // ... bind parameters
    .execute(&self.pool)
    .await
    .map_err(AppError::DatabaseError)?;

// Step 6: Store contacts
if let Some(emails) = work_data.get("emails").and_then(|e| e.as_array()) {
    self.store_party_emails(party_id, emails).await?;
}
if let Some(telefones) = work_data.get("telefones").and_then(|t| t.as_array()) {
    self.store_party_phones(party_id, telefones).await?;
}

// Step 7: Store enrichment snapshot
let quality_score = risk_score
    .as_ref()
    .and_then(|bd| bd.to_string().parse::<f64>().ok())
    .unwrap_or(0.5);

sqlx::query("INSERT INTO core.party_enrichments (...) VALUES (...) ON CONFLICT (party_id) DO UPDATE ...")
    .bind(party_id)
    .bind(&enrichment_payload)
    .bind(quality_score)
    .execute(&self.pool)
    .await
    .map_err(AppError::DatabaseError)?;

Ok(party_id)
```

---

### 7.2 Email Storage Helper

**Function:** `store_party_emails()`

**Location:** `src/db_storage.rs:310-360`

```rust
async fn store_party_emails(
    &self,
    party_id: Uuid,
    emails: &[serde_json::Value],
) -> Result<(), AppError> {
    for (idx, email_obj) in emails.iter().enumerate() {
        let email = email_obj.get("email").and_then(|e| e.as_str());
        let prioridade = email_obj.get("prioridade").and_then(|p| p.as_str());
        let qualidade = email_obj.get("qualidade").and_then(|q| q.as_str());

        if let Some(email_addr) = email {
            let is_primary = idx == 0; // First email is primary
            let is_verified = qualidade == Some("BOM");

            // Build metadata JSONB
            let mut metadata = json!({});
            if let Some(prio) = prioridade {
                metadata["prioridade"] = json!(prio);
            }
            if let Some(qual) = qualidade {
                metadata["qualidade"] = json!(qual);
            }

            let _ = sqlx::query(
                r#"
                INSERT INTO core.party_contacts (
                    contact_id, party_id, contact_type, value,
                    is_primary, is_verified, is_whatsapp, source,
                    confidence, valid_from, valid_to, created_at, updated_at
                )
                VALUES (gen_random_uuid(), $1, 'email'::core.contact_type_enum, $2, $3, $4, false, $5, $6, now(), NULL, now(), now())
                ON CONFLICT ON CONSTRAINT uq_party_contact_unique DO NOTHING
                "#,
            )
            .bind(party_id)
            .bind(email_addr.to_lowercase())
            .bind(is_primary)
            .bind(is_verified)
            .bind(metadata.get("prioridade").and_then(|v| v.as_str()))
            .bind(metadata.get("qualidade").and_then(|v| v.as_str()).and_then(|s| s.parse::<f64>().ok()))
            .execute(&self.pool)
            .await;
        }
    }

    Ok(())
}
```

---

### 7.3 Phone Storage Helper

**Function:** `store_party_phones()`

**Location:** `src/db_storage.rs:365-410`

```rust
async fn store_party_phones(
    &self,
    party_id: Uuid,
    telefones: &[serde_json::Value],
) -> Result<(), AppError> {
    for (idx, phone_obj) in telefones.iter().enumerate() {
        let telefone = phone_obj.get("telefone").and_then(|t| t.as_str());
        let whatsapp = phone_obj.get("whatsapp").and_then(|w| w.as_str());
        let operadora = phone_obj.get("operadora").and_then(|o| o.as_str());
        let status = phone_obj.get("status").and_then(|s| s.as_str());

        if let Some(phone) = telefone {
            let is_primary = idx == 0;
            let is_whatsapp = whatsapp == Some("SIM");
            
            // Normalize phone: digits only
            let normalized: String = phone.chars()
                .filter(|c| c.is_ascii_digit())
                .collect();

            let _ = sqlx::query(
                r#"
                INSERT INTO core.party_contacts (
                    contact_id, party_id, contact_type, value,
                    is_primary, is_verified, is_whatsapp, source,
                    confidence, valid_from, valid_to, created_at, updated_at
                )
                VALUES (
                    gen_random_uuid(), $1,
                    CASE WHEN $3 THEN 'whatsapp'::core.contact_type_enum ELSE 'phone'::core.contact_type_enum END,
                    $2, $4, true, $3, $5, $6, now(), NULL, now(), now()
                )
                ON CONFLICT ON CONSTRAINT uq_party_contact_unique DO NOTHING
                "#,
            )
            .bind(party_id)
            .bind(&normalized)
            .bind(is_whatsapp)
            .bind(is_primary)
            .bind(operadora)
            .bind(status.and_then(|s| s.parse::<f64>().ok()))
            .execute(&self.pool)
            .await;
        }
    }

    Ok(())
}
```

---

### 7.4 Date Parsing Helper

**Function:** `parse_br_date()`

**Location:** `src/db_storage.rs:415-420`

```rust
/// Parse Brazilian date format (DD/MM/YYYY) to chrono::NaiveDate
fn parse_br_date(date_str: &str) -> Result<chrono::NaiveDate, chrono::ParseError> {
    chrono::NaiveDate::parse_from_str(date_str, "%d/%m/%Y")
}
```

**Usage:**

```rust
let data_nasc = dados_basicos
    .and_then(|d| d.get("dataNascimento"))
    .and_then(|v| v.as_str())
    .and_then(|d| parse_br_date(d).ok());  // Returns Option<NaiveDate>
```

---

## 8. Verification Queries

### 8.1 Check if Party Exists by CPF

```sql
SELECT id, full_name, enriched, created_at
FROM core.parties
WHERE cpf_cnpj = '12345678901' 
AND party_type = 'person';
```

**Expected Result:**
```
id                                   | full_name        | enriched | created_at
-------------------------------------|------------------|----------|-------------------------
a1b2c3d4-e5f6-7890-abcd-ef1234567890 | EDSON TARRAF     | true     | 2025-11-20 14:32:10+00
```

---

### 8.2 View All Enriched Data for a CPF

```sql
-- Party + Person
SELECT 
    p.id as party_id,
    p.cpf_cnpj,
    p.full_name,
    p.birth_date,
    p.sex,
    p.mother_name,
    p.enriched,
    pe.marital_status,
    pe.document_cpf
FROM core.parties p
LEFT JOIN core.people pe ON p.id = pe.party_id
WHERE p.cpf_cnpj = '12345678901' 
AND p.party_type = 'person';

-- Contacts
SELECT 
    contact_type,
    value,
    is_primary,
    is_verified,
    is_whatsapp,
    confidence
FROM core.party_contacts
WHERE party_id = (
    SELECT id FROM core.parties WHERE cpf_cnpj = '12345678901' LIMIT 1
)
ORDER BY contact_type, is_primary DESC;

-- Enrichment Snapshot
SELECT 
    provider,
    quality_score,
    enriched_at,
    raw_payload->'DadosBasicos' as dados_basicos,
    raw_payload->'DadosEconomicos' as dados_economicos
FROM core.party_enrichments
WHERE party_id = (
    SELECT id FROM core.parties WHERE cpf_cnpj = '12345678901' LIMIT 1
);
```

---

### 8.3 Check Enrichment Coverage

```sql
-- Overall enrichment rate
SELECT 
    COUNT(*) as total_parties,
    SUM(CASE WHEN enriched THEN 1 ELSE 0 END) as enriched_count,
    ROUND((SUM(CASE WHEN enriched THEN 1 ELSE 0 END)::numeric / COUNT(*)) * 100, 2) as enrichment_rate_pct
FROM core.parties
WHERE party_type = 'person';
```

**Expected Result:**
```
total_parties | enriched_count | enrichment_rate_pct
--------------|----------------|--------------------
    1124247   |     695294     |       61.86
```

---

### 8.4 Find Party by C2S Lead ID

```sql
SELECT 
    p.id as party_id,
    p.cpf_cnpj,
    p.full_name,
    pe.raw_payload->>'lead_id' as c2s_lead_id,
    pe.enriched_at
FROM core.parties p
JOIN core.party_enrichments pe ON p.id = pe.party_id
WHERE pe.raw_payload->>'lead_id' = 'bf1a88eaa4ab34b01a257536563fb42b';
```

---

### 8.5 View Contacts by Type

```sql
-- Emails only
SELECT 
    p.full_name,
    pc.value as email,
    pc.is_primary,
    pc.is_verified,
    pc.confidence
FROM core.party_contacts pc
JOIN core.parties p ON pc.party_id = p.id
WHERE pc.contact_type = 'email'
AND p.cpf_cnpj = '12345678901';

-- Phones only (including WhatsApp)
SELECT 
    p.full_name,
    pc.contact_type,
    pc.value as phone,
    pc.is_primary,
    pc.is_whatsapp,
    pc.source
FROM core.party_contacts pc
JOIN core.parties p ON pc.party_id = p.id
WHERE pc.contact_type IN ('phone', 'whatsapp')
AND p.cpf_cnpj = '12345678901'
ORDER BY pc.is_primary DESC;
```

---

### 8.6 Find Duplicates (Same CPF, Multiple Records)

```sql
-- This should return empty if deduplication is working correctly
SELECT 
    cpf_cnpj,
    COUNT(*) as record_count,
    array_agg(id) as party_ids,
    array_agg(full_name) as names
FROM core.parties
WHERE party_type = 'person'
AND cpf_cnpj IS NOT NULL
GROUP BY cpf_cnpj
HAVING COUNT(*) > 1
ORDER BY record_count DESC;
```

**Note:** Database allows duplicates (no UNIQUE constraint), but application logic prevents them via `SELECT ... WHERE cpf_cnpj = ? LIMIT 1` check.

---

### 8.7 Enrichment Quality Report

```sql
SELECT 
    COUNT(*) as total_enrichments,
    ROUND(AVG(quality_score), 2) as avg_quality_score,
    ROUND(MIN(quality_score), 2) as min_score,
    ROUND(MAX(quality_score), 2) as max_score,
    COUNT(CASE WHEN quality_score >= 0.7 THEN 1 END) as high_quality,
    COUNT(CASE WHEN quality_score BETWEEN 0.4 AND 0.69 THEN 1 END) as medium_quality,
    COUNT(CASE WHEN quality_score < 0.4 THEN 1 END) as low_quality
FROM core.party_enrichments;
```

---

### 8.8 Recent Enrichments (Last 7 Days)

```sql
SELECT 
    p.cpf_cnpj,
    p.full_name,
    pe.provider,
    pe.quality_score,
    pe.enriched_at
FROM core.party_enrichments pe
JOIN core.parties p ON pe.party_id = p.id
WHERE pe.enriched_at >= NOW() - INTERVAL '7 days'
ORDER BY pe.enriched_at DESC
LIMIT 50;
```

---

### 8.9 Contact Statistics

```sql
-- Contacts per party
SELECT 
    contact_type,
    COUNT(*) as total_contacts,
    COUNT(DISTINCT party_id) as unique_parties,
    ROUND(AVG(CASE WHEN confidence IS NOT NULL THEN confidence ELSE 0.5 END), 2) as avg_confidence
FROM core.party_contacts
GROUP BY contact_type;
```

**Expected Result:**
```
contact_type | total_contacts | unique_parties | avg_confidence
-------------|----------------|----------------|---------------
email        |      1245682   |      982341    |      0.72
phone        |       985124   |      756892    |      0.68
whatsapp     |       364050   |      312456    |      0.75
```

---

### 8.10 Missing Data Analysis

```sql
-- Parties with missing key fields
SELECT 
    COUNT(*) as total_parties,
    SUM(CASE WHEN birth_date IS NULL THEN 1 ELSE 0 END) as missing_birth_date,
    SUM(CASE WHEN sex IS NULL THEN 1 ELSE 0 END) as missing_sex,
    SUM(CASE WHEN mother_name IS NULL THEN 1 ELSE 0 END) as missing_mother_name,
    SUM(CASE WHEN enriched = false THEN 1 ELSE 0 END) as not_enriched
FROM core.parties
WHERE party_type = 'person';
```

---

## Appendix A: Related Files

### Implementation Files

| File | Purpose |
|------|---------|
| `src/db_storage.rs` | Core storage logic (this document) |
| `src/handlers.rs` | API endpoints that call storage |
| `src/models.rs` | Rust data models and DTOs |
| `src/services.rs` | Work API integration |
| `src/errors.rs` | Error handling types |

### Documentation Files

| File | Purpose |
|------|---------|
| `docs/database/DATABASE_SCHEMA_REPORT_FINAL.md` | Complete schema documentation |
| `docs/queries/ENRICHMENT_FLOW.md` | High-level flow diagram |
| `docs/queries/work_api_enrichment.sql` | SQL query reference |
| `docs/queries/customers.sql` | Customer lookup queries |
| `docs/integrations/WORK_API_RATE_LIMITING.md` | Rate limiting guide |

### Example/Test Files

| File | Purpose |
|------|---------|
| `examples/batch_enrich.rs` | Batch CPF enrichment example |
| `examples/import_json_to_db.rs` | JSON import utility |
| `scripts/enrich_batch.sh` | Bash script for batch processing |

---

## Appendix B: Performance Considerations

### Database Performance

**Indexes Used:**
- `idx_parties_cpf_cnpj` - Fast CPF lookups (< 10ms)
- `idx_parties_normalized_name` - Name search
- `idx_party_contacts_party` - Contact retrieval by party_id
- `idx_party_contacts_value` - Contact search by value

**Query Performance (p95):**
| Query Type | Performance |
|------------|------------|
| CPF lookup | < 10ms |
| Upsert party | < 20ms |
| Store contacts | < 30ms (batch) |
| Full enrichment | < 100ms |

### Application Performance

**Bottlenecks:**
1. **Work API latency** - 10-60 seconds per request
2. **Sequential queries** - No transaction overhead, but multiple round-trips
3. **JSONB storage** - Large payloads increase storage and retrieval time

**Optimization Opportunities:**
1. Use transactions to reduce round-trips
2. Batch contact inserts (single query with UNNEST)
3. Add connection pooling (already implemented via SQLx)

---

## Appendix C: Future Enhancements

### Planned Features

1. **Transaction Support**
   - Wrap entire enrichment in a transaction for atomicity
   - Rollback on failure to prevent partial enrichment

2. **Address Storage**
   - Currently not implemented in Party Model
   - Need to create `core.party_addresses` table
   - Implement address confidence scoring

3. **Property Ownership**
   - Store real estate data from Work API
   - Link via `core.ownerships` table

4. **Relationship Mapping**
   - Store family/business relationships
   - Use `core.party_relationships` table

5. **Better Metadata Handling**
   - Store more Work API fields in structured metadata
   - Add `metadata` column to `core.parties`

6. **Audit Trail**
   - Track all enrichment changes via `audit.logged_actions`
   - Currently only triggers on table-level changes

7. **Error Handling**
   - Add retry logic for transient database errors
   - Log failed enrichments for manual review

8. **Validation**
   - Add CPF format validation (11 digits, valid check digit)
   - Validate email format before storing
   - Validate phone number format (DDD + number)

---

## Appendix D: Common Issues & Solutions

### Issue: Duplicate Parties Created

**Symptom:** Multiple records with same CPF in `core.parties`

**Cause:** Race condition in concurrent enrichments

**Solution:** 
- Add UNIQUE constraint: `ALTER TABLE core.parties ADD CONSTRAINT uq_parties_cpf_cnpj UNIQUE (cpf_cnpj)`
- Or use database-level locking: `SELECT ... FOR UPDATE`

---

### Issue: Email Not Storing

**Symptom:** Email appears in Work API response but not in database

**Cause:** Duplicate email for same party (constraint violation)

**Solution:** Query existing contacts before declaring missing:

```sql
SELECT value FROM core.party_contacts 
WHERE party_id = ? AND contact_type = 'email';
```

---

### Issue: Date Parse Error

**Symptom:** Birth date not stored, remains NULL

**Cause:** Work API returns invalid or unusual date format

**Solution:** Add error logging to `parse_br_date()`:

```rust
fn parse_br_date(date_str: &str) -> Result<chrono::NaiveDate, chrono::ParseError> {
    chrono::NaiveDate::parse_from_str(date_str, "%d/%m/%Y")
        .map_err(|e| {
            tracing::warn!("Failed to parse date '{}': {}", date_str, e);
            e
        })
}
```

---

### Issue: Enrichment Quality Always 0.5

**Symptom:** All enrichments have `quality_score = 0.5`

**Cause:** Risk level not found in Work API response or mapping failed

**Solution:** Check if Work API returns `DadosEconomicos.score.scoreCSBAFaixaRisco` field, add logging:

```rust
let risk_level = dados_econ
    .and_then(|d| d.get("score"))
    .and_then(|s| s.get("scoreCSBAFaixaRisco"))
    .and_then(|v| v.as_str());

if risk_level.is_none() {
    tracing::warn!("Risk level not found in Work API response for CPF: {}", cpf);
}
```

---

**Document Version:** 1.0  
**Last Updated:** 2025-11-22  
**Author:** Engineering Team  
**Maintained By:** MbInteligen

---

*This guide is generated from production code and database schema. For the latest implementation details, always refer to the source code in `src/db_storage.rs`.*
