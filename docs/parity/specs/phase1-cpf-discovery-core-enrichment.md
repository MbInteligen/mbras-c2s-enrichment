# Phase 1 — CPF Discovery & Core Enrichment — Parity Contract

**Feature key:** `phase1-cpf-discovery`
**Linear issue:** IBVI-341
**Last updated:** 2026-02-14
**Status:** spec_ready

---

## Scope

Phase 1 brings the Rust CPF discovery pipeline to parity with TS by implementing:

1. Work API `phone` module (Tier 1)
2. Work API `name` module (Tier 2)
3. Work API `mail` module (Tier 1 email)
4. CPF mod-11 validation (shared gate)
5. Name matching (Levenshtein + abbreviation expansion)
6. Tier reordering: Work API → DuckDB → Diretrix → DBase
7. Batch endpoint (`POST /batch/enrich-direct`)
8. Retry service with exponential backoff
9. Enrichment cron with advisory lock

---

## 1. CPF Mod-11 Validation

**TS source:** `WorkApiService.isValidCpf()` (static method)
**Rust target:** `src/cpf.rs::is_valid_cpf()`

### Algorithm

```
Input: string of exactly 11 ASCII digits (after normalization)

Step 0: Reject if all digits are the same (e.g. "11111111111")

Step 1 (first check digit, position 9):
  sum = Σ(cpf[i] * (10 - i)) for i = 0..8
  d1 = 11 - (sum % 11)
  if d1 >= 10 then d1 = 0
  if cpf[9] != d1 → INVALID

Step 2 (second check digit, position 10):
  sum = Σ(cpf[i] * (11 - i)) for i = 0..9
  d2 = 11 - (sum % 11)
  if d2 >= 10 then d2 = 0
  if cpf[10] != d2 → INVALID

Result: VALID
```

### Normalization

| Source | Raw format | Normalization |
|--------|-----------|---------------|
| Work API (all modules) | 14-digit with leading zeros | `s[s.len()-11..]` (last 11 chars) |
| CPF Lookup DuckDB | 11-digit | Use as-is |
| Diretrix | 11-digit | Use as-is |
| DBase | 11-digit | Use as-is |
| User input | May have dots/dash | Strip non-digits, then take last 11 |

### Fixture cases (group: `cpf_mod11`)

| case_id | input | expected_valid | notes |
|---------|-------|---------------|-------|
| `valid-standard` | `"52998224725"` | true | Standard valid CPF |
| `valid-leading-zero` | `"01234567890"` → after mod-11 check | depends | Test with actual valid CPF starting with 0 |
| `valid-14-digit` | `"00052998224725"` | true (after normalize to 11) | Work API format |
| `invalid-all-same` | `"11111111111"` | false | All same digits |
| `invalid-check-digit` | `"12345678900"` | false | Wrong check digits |
| `invalid-short` | `"1234567"` | false | Too short |
| `invalid-cnpj` | `"12345678000190"` | false (14 digits = CNPJ) | Must reject |

---

## 2. Phone Discovery — 5-Tier Fallback

**TS source:** `CpfDiscoveryService.findCpfByPhone(phone, leadName?)`
**Rust target:** `src/discovery.rs::find_cpf_by_phone()`

### Tier order

```
Tier 1: Work API phone   → fastest, most reliable for known numbers
Tier 2: Work API name    → requires leadName >= 5 chars, <= 20 results
Tier 3: CPF Lookup DuckDB → 223M records, name-based, ~120s latency
Tier 4: Diretrix          → direct phone API
Tier 5: DBase             → local phone lookup
```

### Tier 1: Work API `phone` module

- **URL:** `GET /api?token={T}&modulo=phone&consulta={PHONE}`
- **Response:** `{ msg: [{ cpf_cnpj: "00028659500857", nome: "..." }, ...] }`
- **CPF normalization:** `.slice(-11)` from 14-char format
- **Validation:** Each result passes `is_valid_cpf()` (rejects CNPJs and invalid CPFs)
- **Selection:** First valid CPF from `msg[]` array
- **Proceed to Tier 2 if:** No valid CPF found in response, timeout, or error

### Tier 2: Work API `name` module

- **URL:** `GET /api?token={T}&modulo=name&consulta={NAME}`
- **Response:** `{ data: [{ cpf: "...", nome: "...", dataNascimento?, nomeMae? }, ...] }`
- **Guard conditions (skip tier if any fail):**
  - `leadName` must be provided
  - `leadName.len() >= 5`
- **Ambiguity guard:** Skip if `data.len() > 20`
- **Selection:** `find_best_match(leadName, candidates, threshold=0.7)`
  - Only accept if `best.score >= 0.7`
- **CPF validation:** Result must pass `is_valid_cpf()`
- **Source tag:** `"work-api-name"`

### Tier 3: CPF Lookup DuckDB (223M records)

- **Endpoint:** `GET https://cpf-lookup-api.fly.dev/search/{NAME}`
- **Guard conditions:**
  - `leadName` must be provided
  - `leadName.len() >= 5`
- **Selection:** `match_names(leadName, result.nome_completo)` with threshold 0.7
- **Auto-scaling:** Scale machine to 16GB before search, 256MB after 5min idle
- **Source tag:** `"cpf-lookup-223m-name"`

### Tier 4: Diretrix

- **Method:** `diretrix_service.search_by_phone(phone)`
- **Source tag:** `"diretrix"`
- **Skip if:** credentials not configured

### Tier 5: DBase

- **Method:** `dbase_service.search_by_phone(phone)`
- **Source tag:** `"dbase"`

### Discovery result type

```rust
pub struct CpfDiscoveryResult {
    pub cpf: String,           // 11-digit normalized
    pub found_name: String,    // Name from database
    pub name_matches: bool,    // leadName matched foundName?
    pub match_score: f64,      // 0.0-1.0 similarity
    pub match_method: String,  // "exact", "fuzzy-full", "first-exact-last-fuzzy", etc.
    pub source: String,        // "work-api", "work-api-name", "cpf-lookup-223m-name", etc.
}
```

---

## 3. Email Discovery — 2-Tier Fallback

**TS source:** `CpfDiscoveryService.findCpfByEmail(email, leadName?)`

### Tier order

```
Tier 1: Work API mail   → searches by email address
Tier 2: Diretrix        → email-based lookup
```

### Tier 1: Work API `mail` module

- **URL:** `GET /api?token={T}&modulo=mail&consulta={EMAIL}`
- **Response:** `{ msg: [{ cpf_cnpj: "...", nome: "...", dataNascimento? }, ...] }`
- **CPF validation:** `is_valid_cpf()` after normalize
- **Disambiguation:** If multiple results and `leadName` provided, select best name match
- **Source tag:** `"work-api-mail"`

### Tier 2: Diretrix

- **Method:** `diretrix_service.search_by_email(email)`
- **Source tag:** `"diretrix"`

---

## 4. Name Matching

**TS source:** `src/utils/name-matcher.ts::matchNames()`
**Rust target:** `src/name_matcher.rs::match_names()`

### Normalization pipeline

```
1. to_uppercase()
2. NFD decompose + strip combining marks (remove accents)
3. Expand abbreviations: MA.→MARIA, JO.→JOSE, ANT.→ANTONIO, etc.
4. Remove suffixes: JUNIOR, JR, FILHO, NETO, SOBRINHO, SEGUNDO, II, III
5. Collapse whitespace
6. Trim
```

### Abbreviation map

```
MA. → MARIA    M. → MARIA    JO. → JOSE     J. → JOSE
ANT. → ANTONIO FCO. → FRANCISCO  DR. → DOUTOR  DRA. → DOUTORA
SR. → SENHOR   SRA. → SENHORA    S. → SANTOS   STO. → SANTO
STA. → SANTA
```

### Match strategies (in order)

| # | Strategy | Score | Condition |
|---|----------|-------|-----------|
| 1 | Exact | 1.0 | Normalized strings equal |
| 2 | Fuzzy-full | Levenshtein similarity | `sim >= threshold` (default 0.75) |
| 3 | First-name-only | 0.85 | First names equal, `first.len >= 3`, lead has only one word |
| 4 | First-exact-last-fuzzy | `(1 + lastSim) / 2` | First names equal, `lastSim >= 0.6` |
| 5 | Last-exact-first-fuzzy | `(1 + firstSim) / 2` | Last names equal, `firstSim >= 0.6` |
| 6 | Contains | `0.7 + ratio * 0.3` | One name contains the other, `ratio >= 0.3` |
| 7 | Abbreviation-match | 0.8 | First names equal, one last name is 1-2 char prefix of other |
| 8 | Initials-match | 0.85 | Lead is 2-3 uppercase chars matching DB name initials |
| 9 | Initials-lastname-match | 0.9 | Lead starts with initials + last name matches |
| 10 | No-match | fullSim | None of the above |

### Levenshtein similarity

```
similarity(a, b) = 1 - levenshtein_distance(a, b) / max(a.len, b.len)
```

### find_best_match

```rust
pub fn find_best_match(
    lead_name: &str,
    candidates: &[(String, String)],  // (name, cpf)
    threshold: f64,                    // default 0.75
) -> Option<(String, String, f64, String)>  // (name, cpf, score, method)
```

Iterates all candidates, returns highest-scoring match above threshold.

---

## 5. Work API Rate Limiting

**Constraint:** Minimum 2 seconds between any two Work API requests.

**TS implementation:** Static `lastRequestTime` timestamp, checked before each call.

**Rust implementation:** `tokio::time::sleep()` to enforce minimum gap.

### Timeouts

| Module | Timeout |
|--------|---------|
| `cpf` | 30 seconds |
| `phone` | 15 seconds |
| `name` | 15 seconds |
| `mail` | 15 seconds |

### Retry (Work API CPF module only)

- Max 3 retries
- Backoff: 1s, 2s, 4s
- Don't retry on: timeout, 404, abort
- Do retry on: network errors, 5xx

---

## 6. Enrichment Status Lifecycle

```
pending → processing → completed | partial | basic | unenriched
                                    ↓ (retry)
                        completed | partial | basic | failed
```

### Status definitions

| Status | Meaning |
|--------|---------|
| `pending` | Received, not yet processed |
| `processing` | Currently being enriched |
| `completed` | CPF found + Work API enrichment data |
| `partial` | CPF found but Work API timed out (enrichable later) |
| `basic` | CPF found but Work API returned minimal data |
| `unenriched` | No CPF found across all tiers |
| `failed` | Max retries exceeded (terminal) |

### Retryable statuses

```
["partial", "unenriched", "basic"]
```

---

## 7. Retry Service

### Exponential backoff

| Attempt | Delay |
|---------|-------|
| 0 | 1 hour |
| 1 | 2 hours |
| 2 | 4 hours |
| 3 | 8 hours |
| 4 | 16 hours |

**Max retries:** 5 (configurable)

### Eligibility check

```
is_retry_eligible(lead) =
  lead.status IN RETRYABLE_STATUSES
  AND lead.retry_count < MAX_RETRIES
  AND (lead.last_retry_at IS NULL
       OR now() - lead.last_retry_at >= backoff_delay(lead.retry_count))
```

---

## 8. Batch Endpoint

### POST /batch/enrich-direct

**Purpose:** Direct enrichment without C2S integration.

**Request:**
```json
{
  "phone": "11999887766",      // at least one of phone/email required
  "email": "user@example.com", // optional
  "name": "João Silva"         // optional, improves name-based discovery
}
```

**Response:**
```json
{
  "status": "completed",       // completed | partial | unenriched
  "cpf": "12345678901",
  "cpfSource": "work-api",
  "name": "JOAO SILVA",
  "matchScore": 0.95,
  "data": { ... }              // Work API person data (when completed)
}
```

**Uses:** Full 5-tier CPF discovery pipeline.

---

## 9. Income Multiplier

**Constant:** `INCOME_MULTIPLIER = 1.9`

**Applied to:** Display/derived income fields only. Raw API values stored unchanged.

```rust
let display_income = raw_income * INCOME_MULTIPLIER;
```

---

## 10. Constants & Thresholds

| Constant | Value | Usage |
|----------|-------|-------|
| `WORK_API_RATE_LIMIT_MS` | 2000 | Min gap between Work API calls |
| `WORK_API_CPF_TIMEOUT_S` | 30 | Timeout for modulo=cpf |
| `WORK_API_OTHER_TIMEOUT_S` | 15 | Timeout for phone/name/mail |
| `NAME_MIN_LENGTH` | 5 | Min chars for name-based discovery |
| `NAME_MATCH_THRESHOLD` | 0.7 | Min score for CPF discovery acceptance |
| `NAME_MATCH_DEFAULT_THRESHOLD` | 0.75 | Default for matchNames function |
| `WORK_NAME_MAX_RESULTS` | 20 | Max results from name module before rejecting |
| `INCOME_MULTIPLIER` | 1.9 | Display income = raw × 1.9 |
| `MAX_RETRY_ATTEMPTS` | 5 | Terminal failure threshold |
| `BATCH_DEFAULT_DELAY_MS` | 500 | Default delay between batch leads |
| `BATCH_MIN_DELAY_MS` | 100 | Min delay |
| `BATCH_MAX_DELAY_MS` | 5000 | Max delay |

---

## Fixture Mapping

Fixture file: `docs/parity/fixtures/phase1-cpf-discovery.json`

### Case groups

| Group | Cases | Tests |
|-------|-------|-------|
| `cpf_mod11` | 7 | Validation: valid, invalid, CNPJ, normalize |
| `name_matching` | 10 | All 10 match strategies + edge cases |
| `phone_discovery` | 5 | Tier 1-5 fallback scenarios |
| `email_discovery` | 3 | Tier 1-2 email scenarios |
| `income_multiplier` | 3 | Raw → display transformation |
| `retry_policy` | 4 | Backoff timing, eligibility, max retries |

---

## Invariants

1. Discovery never returns CNPJ as CPF
2. Discovery never returns CPF failing mod-11 validation
3. Lower tiers evaluated only after higher tiers fail/skip
4. `source` field accurately reflects the tier that produced the accepted CPF
5. Income multiplier applied only to display values, never to stored raw values
6. Retry respects backoff windows and max retry count
7. Advisory lock prevents duplicate enrichment across instances
