# Phase 3 — Lead Scoring Parity Contract

**Feature key:** `phase3-lead-scoring`
**Linear issue:** RML-1094
**TS source files:**
- `src/services/lead-quality.service.ts` (310 lines)
- `src/utils/high-value-detector.ts` (268 lines)
- `src/services/tier-calculator.service.ts` (376 lines)
- `src/utils/neighborhoods.ts` (114 lines)
- `src/utils/surname-analyzer.ts` (414 lines)

**Rust target modules:**
- `src/scoring/quality.rs`
- `src/scoring/high_value.rs`
- `src/scoring/tier.rs`
- `src/scoring/neighborhoods.rs`
- `src/scoring/families.rs`

---

## Module 1: Lead Quality Score (`quality.rs`)

### Input

| Field | TS type | Rust type | Notes |
|-------|---------|-----------|-------|
| name | `string?` | `Option<String>` | |
| phone | `string?` | `Option<String>` | |
| email | `string?` | `Option<String>` | |
| cpf | `string?` | `Option<String>` | |
| enrichedName | `string?` | `Option<String>` | |
| income | `number?` | `Option<f64>` | `None` = missing, `Some(0.0)` = explicit zero |
| presumedIncome | `number?` | `Option<f64>` | |
| addresses | `Array<{neighborhood?,city?,state?}>` | `Vec<Address>` | |
| companyCount | `number?` | `Option<u32>` | u32 prevents negative |
| totalCompanyCapital | `number?` | `Option<f64>` | |
| isCompanyAdministrator | `boolean?` | `bool` | defaults false |
| hasRealEstateSector | `boolean?` | `bool` | defaults false |

### Output

| Field | Type | Range |
|-------|------|-------|
| score | u32 | 0-100 |
| grade | Grade | A/B/C/D/F |
| category | Category | premium/high/standard/low/poor |
| scoreMethod | ScoreMethod | direct/inferred/none |
| breakdown.dataCompleteness | u32 | 0-30 |
| breakdown.incomeScore | u32 | 0-25 |
| breakdown.locationScore | u32 | 0-15 |
| breakdown.contactValidity | u32 | 0-20 |
| breakdown.enrichmentBonus | u32 | 0-10 |
| flags | Vec<String> | |
| recommendations | Vec<String> | |

### Scoring Rules

**Data Completeness (max 30):**
- Name: full (3+ words, 10+ chars) = 10, partial (2+ words, 5+ chars) = 7, minimal (3+ chars) = 3
- Phone: valid DDD + length = 10, short (8+) = 5, invalid DDD = 3
- Email: valid format = 5, invalid = 2
- CPF: present = 5

**Income (max 25):**
- Direct: R$20k+ = 25, R$15k+ = 20, R$10k+ = 15, R$5k+ = 10, R$3k+ = 5
- Inferred (only when income is `None`): capital >= 5M → +15, >= 1M → +10, >= 100K → +5; admin → +5; realEstate → +5; cap at 25
- `income = Some(0)` → direct, score = 0 (NOT inferred)

**Location (max 15):**
- Noble neighborhood = 15, regular neighborhood = 8, address only = 5
- SP/RJ capital bonus: +2 (capped at 15)

**Contact Validity (max 20):**
- Valid mobile (DDD + 9-digit) = 15, valid landline = 10
- Premium email domain = 5, corporate = 3

**Enrichment Bonus (max 10):**
- CPF + enriched name = 5
- 1+ companies = 3, 3+ companies = +2

**Grade thresholds:** A >= 90, B >= 70, C >= 50, D >= 30, F < 30

**Valid DDDs:** 11-19, 21-22, 24, 27-28, 31-35, 37-38, 41-46, 47-49, 51, 53-55, 61-69, 71, 73-75, 77, 79, 81-89, 91-99

### Spam Detection

14 regex patterns: `painel\s*fama`, `sucesso\s*com\s*vendas`, `ganhe\s*dinheiro`, `renda\s*extra`, `trabalhe\s*em\s*casa`, `marketing\s*digital`, `afiliado`, `curso\s*online`, `investimento`, `cripto`, `bitcoin`, `forex`, `teste\s*teste`, `^teste$`

If spam detected: score=0, grade=F, category=poor, scoreMethod=none.

---

## Module 2: High-Value Detector (`high_value.rs`)

### Input

| Field | Rust type | Notes |
|-------|-----------|-------|
| income | `Option<f64>` | None=missing, Some(0)=explicit zero |
| presumedIncome | `Option<f64>` | |
| neighborhood | `Option<String>` | |
| addresses | `Vec<Address>` | |
| companyCount | `Option<u32>` | |
| leadName | `Option<String>` | |
| enrichedName | `Option<String>` | |
| propertyCount | `Option<u32>` | |
| propertyValue | `Option<f64>` | |
| netWorth | `Option<f64>` | |
| occupation | `Option<String>` | |
| education | `Option<String>` | |
| totalCompanyCapital | `Option<f64>` | |
| isCompanyAdministrator | `bool` | |
| hasRealEstateSector | `bool` | |

### Scoring

| Factor | Points | Condition |
|--------|--------|-----------|
| Income (high) | 50 | >= R$20k |
| Income (medium) | 36 | >= R$15k |
| Income (low) | 10 | >= R$10k |
| Noble neighborhood | 15 | in SP/RJ noble list |
| Companies (3+) | 20 | companyCount >= 3 |
| Notable family | 50 | surname in NOTABLE_FAMILIES |
| Rare surname | 10 | surname in RARE_SURNAMES, confidence >= 80 |
| Properties (2+) | 15 | propertyCount >= 2 |
| Property value (5M+) | 40 | propertyValue >= 5M |
| Property value (2M+) | 25 | propertyValue >= 2M |
| Net worth (5M+) | 45 | netWorth >= 5M |
| Net worth (1M+) | 30 | netWorth >= 1M |
| Executive | 15 | occupation matches executive keywords |
| Professional | 10 | occupation matches professional keywords |
| Education | 5 | post-grad/MBA/etc |
| Company capital (5M+) | 40 | totalCompanyCapital >= 5M |
| Company capital (1M+) | 25 | totalCompanyCapital >= 1M |
| Company capital (500K+) | 15 | totalCompanyCapital >= 500K |
| Administrator | 10 | isCompanyAdministrator |

### Missing-Income Adjustment

When `hasIncomeData == false` AND `score >= 25`:
1. Count independent signal categories:
   - Location: noble neighborhood present
   - Business: companyCount >= 2
   - Property: propertyValue >= 2M
   - Wealth: netWorth >= 1M
2. If categories >= 2: add `min(categories * 5, 15)` points
3. Set `incomeInferred = true`

### Tiers

| Tier | Score |
|------|-------|
| platinum | >= 60 |
| gold | >= 50 |
| silver | >= 25 |
| none | < 25 |

---

## Module 3: Tier Calculator (`tier.rs`)

### Input

Uses `TierCalculatorService::calculate(name, phone?, email?, enrichmentData?, analysisData?)`

### Scoring Steps (10 steps)

1. **Surname analysis** — notable family: +25, rare surname (confidence > 60): +10
2. **International phone** — +10 if non-Brazilian
3. **Income** — direct: R$30k+ = +25, R$15k+ = +15; inferred (when income None): capital >= 5M → +20, >= 1M → +15, >= 100K → +5; admin → +5; cap at 25
4. **Neighborhood** — noble: +15
5. **Properties** — 3+: +5
6. **Domain/company** — high-value sector: +15
7. **Person info** — high-value role: +15; elite education: +20; Brazilian elite: +10
8. **Discovered companies** — 2+: +10 (business owner) + +10 (multiple); 1: +10 (business owner only)
9. **Managed capital** — VC/PE indicator: +35
10. **Risk** — critical: -100, high: -50, medium: -30, low: -10

### Tier Thresholds

| Tier | Score | Override |
|------|-------|---------|
| platinum | >= 70 | |
| gold | >= 50 | |
| silver | >= 30 | |
| bronze | < 30 | |
| risk | any | critical/high risk forces risk tier |

Score clamped to 0-100 after all adjustments.

---

## Shared Data: Neighborhoods

### São Paulo (~48 entries including accent variants)

Jardim Europa, Jardim America, Jardim Paulista, Jardim Paulistano, Jardins, Itaim Bibi, Itaim, Vila Nova Conceicao/Conceição, Moema, Vila Olimpia/Olímpia, Pinheiros, Alto de Pinheiros, Alto Pinheiros, Higienopolis/Higienópolis, Perdizes, Pacaembu, Morumbi, Cidade Jardim, Real Parque, Brooklin, Brooklin Novo, Campo Belo, Vila Mariana, Paraiso/Paraíso, Consolacao/Consolação, Cerqueira Cesar/César, Bela Vista, Butanta/Butantã, Alphaville, Tambore/Tamboré

### Rio de Janeiro (~20 entries including accent variants)

Leblon, Ipanema, Gavea/Gávea, Jardim Botanico/Botânico, Lagoa, Humaita/Humaitá, Botafogo, Flamengo, Laranjeiras, Cosme Velho, Urca, Copacabana, Leme, Barra da Tijuca, Sao Conrado/São Conrado, Joatinga

**Matching:** case-insensitive via `.to_lowercase()`. Accent variants stored as separate entries.

---

## Shared Data: Notable Families

~32 entries (see `surname-analyzer.ts` NOTABLE_FAMILIES map). Each entry: `surname → context string`.

**TOO_COMMON_FOR_NOTABLE:** ~14 surnames that are too common to flag even if in the map (camargo, andrade, batista, etc.)

**RARE_SURNAMES:** ~45 entries (Italian, German, Arab, Japanese, Korean, Chinese, Indian, Jewish, Other).

**COMMON_SURNAMES:** ~45 entries (silva, santos, oliveira, souza, etc.)

---

## Tri-State Income Encoding (Fixtures)

JSON fixtures use explicit envelope to distinguish undefined from zero:

| State | JSON | TS | Rust |
|-------|------|----|------|
| Missing | `{ "state": "missing" }` | `undefined` | `None` |
| Zero | `{ "state": "value", "value": 0 }` | `0` | `Some(0.0)` |
| Data | `{ "state": "value", "value": 80000 }` | `80000` | `Some(80000.0)` |

---

## Edge Cases

1. **income=0 vs income=undefined** — `0` is explicit data (direct scoring, score=0), `undefined` enables proxy
2. **Company count negative** — TS allows it (number), Rust prevents via u32
3. **Score overflow** — TS trusts bucket math; Rust clamps with `.min(100)`
4. **Empty name** — treated as missing, gets 0 data completeness points
5. **Multiple noble neighborhoods** — only best score counts (max across addresses)
6. **Spam + high income** — spam detected = score 0, income irrelevant
