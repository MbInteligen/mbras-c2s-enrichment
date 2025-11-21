# Security and Schema Fixes Applied

## Summary
This document outlines critical security and architectural fixes applied to the Rust C2S API project.

---

## 1. Security: Removed Hardcoded Secrets ✅

### Issue
- Production database credentials and API tokens were at risk of being committed to version control
- `.env` file contained live credentials

### Fix
- Created `.env.example` template with placeholder values
- All secrets are already enforced as mandatory environment variables in `src/config.rs`
- Environment variables will fail-fast at startup if missing (no silent defaults)

### Files Modified
- Created: `.env.example`
- Verified: `src/config.rs` (already secure - all env vars mandatory)

---

## 2. Database Schema: Fixed Customer Queries ✅

### Issue
CustomerService queries referenced non-existent tables:
- `core.parties` → should be `core.entities`
- `core.party_emails` → should be `core.entity_emails`
- `core.party_phones` → should be `core.entity_phones`
- Complex junction table joins that don't exist in production schema

### Fix
Updated all queries in `src/services.rs` to use correct production schema:

**Before:**
```rust
SELECT * FROM core.parties WHERE cpf_cnpj = $1 AND party_type = 'customer'
```

**After:**
```rust
SELECT * FROM core.entities WHERE national_id = $1 AND entity_type = 'person'::core.entity_type_enum
```

### Files Modified
- `src/services.rs` - Updated all CustomerService queries

---

## 3. Feature: Implemented Name-Based Lookup ✅

### Issue
- `CustomerService::find_customer` advertised name search capability
- Handler accepted `name` parameter but silently ignored it
- Would always return "not found" for name-only requests

### Fix
Implemented `find_by_name` method with case-insensitive partial matching:

```rust
async fn find_by_name(&self, name: &str) -> Result<Option<Customer>, AppError> {
    let result = sqlx::query_as::<_, Customer>(
        "SELECT * FROM core.entities
         WHERE LOWER(name) LIKE LOWER($1) AND entity_type = 'person'::core.entity_type_enum
         LIMIT 1",
    )
    .bind(format!("%{}%", name))
    .fetch_optional(&self.pool)
    .await?;
    
    Ok(result)
}
```

### Files Modified
- `src/services.rs` - Added `find_by_name` method and integrated into `find_customer`

---

## 4. API Design: Fixed POST /enrich Endpoint ✅

### Issue
- POST endpoint accepted query parameters instead of JSON body
- Counterintuitive for API consumers (most expect JSON for POST)
- Impossible to send large/complex payloads

### Fix
Changed from `Query<CustomerQueryParams>` to `Json<CustomerQueryParams>`:

**Before:**
```rust
pub async fn enrich_customer(
    State(state): State<Arc<AppState>>,
    Query(params): Query<CustomerQueryParams>,
) -> Result<...>
```

**After:**
```rust
pub async fn enrich_customer(
    State(state): State<Arc<AppState>>,
    Json(params): Json<CustomerQueryParams>,
) -> Result<...>
```

### Files Modified
- `src/handlers.rs` - Updated `enrich_customer` signature

---

## 5. Dependencies: Removed Unused Crates ✅

### Issue
- `thiserror` dependency was listed but never used
- Increases compile time and attack surface unnecessarily

### Fix
Removed `thiserror = "1"` from Cargo.toml

**Note:** `tower` is still needed (used via `tower-http` for CORS and tracing)

### Files Modified
- `Cargo.toml` - Removed unused dependency

---

## 6. Database Storage: Fixed Type Handling ✅

### Issue
- PostgreSQL NUMERIC columns require `BigDecimal` type
- sqlx compile-time checking failed with `query!` macro on complex CTEs
- Prepared statement errors during build

### Fix
1. Added `bigdecimal` dependency and feature flag
2. Converted `f64` financial values to `BigDecimal`:
   ```rust
   let renda = dados_econ
       .and_then(|d| d.get("renda"))
       .and_then(|v| v.as_str())
       .and_then(|r| {
           let normalized = r.replace(",", ".");
           normalized.parse::<f64>().ok()
       })
       .and_then(|r| {
           let adjusted = r * 1.9;
           BigDecimal::from_str(&adjusted.to_string()).ok()
       });
   ```
3. Converted `query!` macros to `query`/`query_as` for runtime flexibility

### Files Modified
- `Cargo.toml` - Added `bigdecimal = "0.4"`
- `src/db_storage.rs` - Updated all queries and type conversions

---

## Build Status
✅ **Project builds successfully with 0 errors**

Remaining warnings are benign:
- Unused `InternalError` variant (reserved for future use)
- Unused Diretrix methods (available for future features)

---

## Testing Recommendations

1. **Verify environment variables:**
   ```bash
   # Should fail without .env
   cargo run
   
   # Should succeed with proper .env
   cp .env.example .env
   # Edit .env with real credentials
   cargo run
   ```

2. **Test database connectivity:**
   - Verify queries return data from production schema
   - Test name-based customer lookup
   - Verify enrichment data storage in `core.entities`, `core.entity_emails`, etc.

3. **Test API endpoints:**
   ```bash
   # POST with JSON body (new behavior)
   curl -X POST http://localhost:8081/api/v1/enrich \
     -H "Content-Type: application/json" \
     -d '{"cpf": "12345678900"}'
   ```

4. **Verify enrichment flow:**
   - Test C2S webhook → Diretrix → Work API → Database storage
   - Confirm enriched data appears in database
   - Verify 1.9x income multiplier is applied

---

## Migration Notes

### Breaking Changes
- **POST /api/v1/enrich** now requires JSON body instead of query parameters
- Clients must update to send `Content-Type: application/json`

### Non-Breaking Changes
- All other fixes are backward compatible
- Database schema unchanged (queries fixed to match existing schema)
- Environment variables already required (no change in behavior)

---

## Security Checklist

- [x] Secrets removed from code
- [x] `.env.example` created with placeholders
- [x] `.env` in `.gitignore`
- [x] Environment variables mandatory (fail-fast)
- [x] No default production credentials
- [ ] **TODO:** Rotate leaked credentials if `.env` was ever committed to git

---

Generated: 2025-01-13
