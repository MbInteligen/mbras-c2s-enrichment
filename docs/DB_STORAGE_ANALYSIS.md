# Database Storage Analysis for Work API Enrichment Data

## Current Database Schema

### Existing Tables
1. **core.parties** - Main entity table (people/companies)
2. **app.emails** - Email addresses
3. **app.phones** - Phone numbers
4. **core.party_emails** - Junction table (party ↔ email)
5. **core.party_phones** - Junction table (party ↔ phone)
6. **app.iptus** - Property tax records
7. **core.party_iptus** - Junction table (party ↔ iptu)

### Current Party Fields
```sql
core.parties:
- id (uuid)
- party_type (text) - "person" or "company"
- cpf_cnpj (text) - indexed
- full_name (text)
- normalized_name (text)
- sex (char(1))
- birth_date (date)
- mother_name (text)
- father_name (text)
- rg (text)
- fantasy_name (text)
- normalized_fantasy_name (text)
- opening_date (date)
- registration_status_date (date)
- company_type (text)
- company_size (text)
- enriched (boolean) - flag for enrichment status
- created_at (timestamp)
- updated_at (timestamp)
```

---

## Work API Data Available

### DadosBasicos (Basic Data)
✅ **Already mapped:**
- cpf → `cpf_cnpj`
- nome → `full_name`
- sexo → `sex`
- dataNascimento → `birth_date`
- nomeMae → `mother_name`
- nomePai → `father_name`

❌ **NOT stored (missing fields):**
- cns (Cartão Nacional de Saúde)
- cor (race/color)
- escolaridade (education level)
- estadoCivil (marital status)
- municipioNascimento (birth city)
- nacionalidade (nationality)
- obito (death info: obito, dataObito)
- situacaoCadastral (registration status: code, description, date)
- conjuge (spouse: nome, cpf, nomeMae, renda)

### DadosEconomicos (Economic Data)
❌ **NOT stored (missing tables):**
- renda (income)
- poderAquisitivo (purchasing power):
  - codigoPoderAquisitivo
  - poderAquisitivoDescricao
  - rendaPoderAquisitivo
  - codigoFaixaRenda
  - faixaPoderAquisitivo
- score (credit scores):
  - scoreCSB
  - scoreCSBFaixaRisco
  - scoreCSBA
  - scoreCSBAFaixaRisco
- serasaMosaic (market segmentation):
  - codigoMosaic
  - descricaoMosaic
  - classeMosaic
  - codigoMosaicNovo
  - descricaoMosaicNovo
  - classeMosaicNovo

### Emails
✅ **Partially mapped:**
- Email addresses stored in `app.emails`
- Junction table `core.party_emails` has `ranking` and `verified` fields

❌ **Missing fields:**
- prioridade (priority: MUITO ALTA, ALTA, MEDIA)
- qualidade (quality: BOM, POTENCIALMENTE BOM, RUIM)
- emailPessoal (is personal email)
- blacklist (is blacklisted)

### Telefones (Phones)
✅ **Partially mapped:**
- Phone numbers stored in `app.phones`
- Junction table `core.party_phones` has `ranking` and `is_whatsapp` fields

❌ **Missing fields:**
- status (phone status)
- tipo (type: TELEFONE RESIDENCIAL, TELEFONE MÓVEL)
- operadora (carrier/operator)

### Endereços (Addresses)
❌ **NOT stored (missing table entirely):**
- tipoLogradouro (street type)
- logradouro (street name)
- logradouroNumero (street number)
- complemento (complement)
- bairro (neighborhood)
- cidade (city)
- uf (state)
- cep (postal code)

### Empresas (Companies)
❌ **NOT stored (missing table entirely):**
- cnpj (company tax ID)
- tipoRelacao (relationship type: QSA, REPRESENTANTELEGAL)
- relacao (relationship: OWNER, etc.)
- admissao (admission date)
- demissao (exit date)

### Other Available Data (NOT stored)
- **profissao** (profession):
  - cbo (occupation code)
  - cboDescricao (occupation description)
  - pis (PIS number)

- **empregos** (employment history) - array

- **registroGeral** (RG details)

- **tituloEleitor** (voter registration):
  - tituloEleitorNumero
  - zonaTitulo
  - secaoTitulo

- **parentes** (relatives):
  - nomeParente
  - cpfParente
  - grauParentesco

- **DadosImposto** (tax data):
  - cpf
  - banco
  - agencia
  - lote
  - ano
  - dataLote
  - status

- **beneficios** (government benefits):
  - auxilioEmergencial
  - bolsaFamilia
  - bpc
  - inss

- **listaDocumentos** (documents):
  - CNS details

- **imunoBiologicos** (vaccination records)

- **pep** (politically exposed persons) - array

- **vizinhos** (neighbors) - array of people

- **internet** (internet usage) - array

- **comprasId** (recent purchases) - array

- **perfilConsumo** (consumption profile):
  - Multiple boolean flags (possui_luxo, possui_investimentos, etc.)
  - Probability percentages for various products/services

- **servidor_siape** (public servant data)

- **flags** (boolean flags):
  - __pessoa_exposta_politicamente__
  - __beneficiario_auxilios__
  - __servidor_publico_siape__

---

## What's Possible RIGHT NOW (No Schema Changes)

### ✅ Can Store Immediately:
1. **Basic Personal Info** → `core.parties`
   - CPF, name, sex, birth_date, mother_name, father_name
   - Set `enriched = true` after enrichment
   - Set `party_type = 'person'`

2. **Emails** → `app.emails` + `core.party_emails`
   - Store email addresses
   - Use `ranking` field for priority
   - Use `verified` field (map from quality?)

3. **Phones** → `app.phones` + `core.party_phones`
   - Store phone numbers
   - Use `ranking` field
   - Use `is_whatsapp` field

### ⚠️ Limitations Without Schema Changes:
- Cannot store RG details (only RG number, no issuer/date)
- Cannot store marital status
- Cannot store education level
- Cannot store nationality
- Cannot store addresses (no addresses table!)
- Cannot store economic data (income, credit score, purchasing power)
- Cannot store employment/company relationships
- Cannot store relatives
- Cannot store detailed email metadata (priority, quality)
- Cannot store detailed phone metadata (type, operator, status)

---

## What Needs to Be Done for Full Storage

### 1. Extend `core.parties` Table
```sql
ALTER TABLE core.parties ADD COLUMN IF NOT EXISTS:
- cns text
- education_level text
- marital_status text
- birth_city text
- nationality text
- death_date date
- is_deceased boolean
- cadastral_status text
- cadastral_status_code text
- cadastral_status_date date
```

### 2. Create `core.party_economic_data` Table
```sql
CREATE TABLE core.party_economic_data (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    party_id uuid REFERENCES core.parties(id),
    income numeric(12,2),
    purchasing_power_code text,
    purchasing_power_description text,
    income_range_code text,
    income_range_description text,
    credit_score_csb integer,
    credit_score_csb_risk text,
    credit_score_csba integer,
    credit_score_csba_risk text,
    mosaic_code text,
    mosaic_description text,
    mosaic_class text,
    mosaic_new_code text,
    mosaic_new_description text,
    mosaic_new_class text,
    created_at timestamp DEFAULT now(),
    updated_at timestamp DEFAULT now(),
    UNIQUE(party_id)
);
```

### 3. Create `app.addresses` Table
```sql
CREATE TABLE app.addresses (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    street_type text,
    street_name text,
    street_number text,
    complement text,
    neighborhood text,
    city text,
    state text,
    postal_code text,
    created_at timestamp DEFAULT now()
);

CREATE TABLE core.party_addresses (
    party_id uuid REFERENCES core.parties(id),
    address_id uuid REFERENCES app.addresses(id),
    ranking integer,
    is_primary boolean DEFAULT false,
    PRIMARY KEY (party_id, address_id)
);
```

### 4. Extend `core.party_emails` Table
```sql
ALTER TABLE core.party_emails ADD COLUMN IF NOT EXISTS:
- priority text  -- MUITO ALTA, ALTA, MEDIA
- quality text   -- BOM, POTENCIALMENTE BOM, RUIM
- is_personal boolean
- is_blacklisted boolean
```

### 5. Extend `core.party_phones` Table
```sql
ALTER TABLE core.party_phones ADD COLUMN IF NOT EXISTS:
- phone_type text      -- TELEFONE RESIDENCIAL, TELEFONE MÓVEL
- phone_status text    -- status from API
- carrier text         -- operadora
```

### 6. Create `core.party_companies` Table
```sql
CREATE TABLE core.party_companies (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    party_id uuid REFERENCES core.parties(id), -- person
    company_cnpj text,
    relationship_type text, -- QSA, REPRESENTANTELEGAL
    relationship text,      -- OWNER, etc.
    admission_date date,
    exit_date date,
    created_at timestamp DEFAULT now()
);
```

### 7. Create `core.party_relatives` Table
```sql
CREATE TABLE core.party_relatives (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    party_id uuid REFERENCES core.parties(id),
    relative_name text,
    relative_cpf text,
    relationship text, -- FILHA(O), MAE, PAI, etc.
    created_at timestamp DEFAULT now()
);
```

### 8. Create `core.party_profession` Table
```sql
CREATE TABLE core.party_profession (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    party_id uuid REFERENCES core.parties(id),
    cbo_code text,
    cbo_description text,
    pis text,
    created_at timestamp DEFAULT now(),
    UNIQUE(party_id)
);
```

### 9. Create `core.party_enrichment_metadata` Table
```sql
CREATE TABLE core.party_enrichment_metadata (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    party_id uuid REFERENCES core.parties(id),
    enrichment_source text, -- 'work_api', 'diretrix', etc.
    enriched_at timestamp DEFAULT now(),
    raw_data jsonb, -- Store full API response
    UNIQUE(party_id, enrichment_source)
);
```

---

## Recommended Approach

### Phase 1: Minimal Implementation (Store Basic Data)
**What:** Use existing tables only
**Stores:**
- Basic personal info (CPF, name, birth date, parents)
- Emails (without metadata)
- Phones (with is_whatsapp flag)
- Set `enriched = true` flag

**Code Changes Needed:**
1. Create database service in `src/services.rs`
2. Add UPSERT logic for parties
3. Add INSERT logic for emails/phones
4. Call after Work API enrichment

### Phase 2: Economic Data (Most Valuable)
**What:** Add economic data table
**Stores:**
- Income
- Credit scores
- Purchasing power
- Mosaic segmentation

**Schema Changes:**
- Create `core.party_economic_data` table

### Phase 3: Addresses
**What:** Add addresses table
**Stores:**
- Full address details with ranking

**Schema Changes:**
- Create `app.addresses` table
- Create `core.party_addresses` junction table

### Phase 4: Extend Metadata
**What:** Add missing fields to existing tables
**Stores:**
- Email priority/quality
- Phone type/carrier
- Extended personal data (education, marital status, etc.)

**Schema Changes:**
- ALTER existing tables to add columns

### Phase 5: Relationships & History
**What:** Add company relationships, relatives, profession
**Stores:**
- Company ownership/management positions
- Family relationships
- Professional information

**Schema Changes:**
- Create relationship tables

---

## Priority Recommendation

**HIGH VALUE:**
1. ✅ Basic personal data (no changes needed)
2. 🟡 Economic data (credit score, income) - **HIGH BUSINESS VALUE**
3. 🟡 Addresses - **HIGH BUSINESS VALUE**

**MEDIUM VALUE:**
4. 🟠 Extended email/phone metadata
5. 🟠 Company relationships

**LOW VALUE:**
6. ⚪ Relatives
7. ⚪ Profession details
8. ⚪ Raw JSON storage for audit

---

## Implementation Complexity

### Easy (1-2 hours):
- Store basic data in existing tables
- Set enriched flag
- Store emails/phones

### Medium (3-5 hours):
- Create economic data table
- Create addresses table
- Implement UPSERT logic with conflict resolution

### Complex (1-2 days):
- Full schema extension
- Migration scripts
- Data validation
- Error handling for partial enrichment
- Rollback strategies

---

## Storage Size Estimation

Per enriched person (full data):
- core.parties: ~500 bytes
- Economic data: ~300 bytes
- Addresses (avg 2): ~400 bytes
- Emails (avg 3): ~200 bytes
- Phones (avg 5): ~250 bytes
- Companies (avg 1): ~200 bytes
- Relatives (avg 3): ~300 bytes
- **Total: ~2.1 KB per person**

For 100,000 enriched people: ~210 MB
For 1,000,000 enriched people: ~2.1 GB

**Conclusion:** Storage is not a concern. The data is relatively small even at scale.
