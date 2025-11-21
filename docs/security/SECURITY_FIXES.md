# Security Fixes Applied

## ✅ Issues Fixed

### 1. Removed Hard-coded Credentials from Config
**File**: `src/config.rs`

**Before**:
```rust
database_url: std::env::var("DB_URL")
    .unwrap_or_else(|_| {
        "postgresql://neondb_owner:npg_xDdKzl0M2TAN@...".to_string()
    }),
c2s_token: std::env::var("C2S_TOKEN")
    .unwrap_or_else(|_| "4ecfcda34202be88a3f8ef70a79b097035621cca7dfe36b8b3".to_string()),
worker_api_key: std::env::var("WORK_API")
    .unwrap_or_else(|_| "zuZKCfxQqGMYbIKKaIDvzgdq".to_string()),
```

**After**:
```rust
database_url: std::env::var("DB_URL")
    .or_else(|_| std::env::var("DATABASE_URL"))
    .map_err(|_| anyhow::anyhow!("DB_URL or DATABASE_URL environment variable required"))?,
c2s_token: std::env::var("C2S_TOKEN")
    .map_err(|_| anyhow::anyhow!("C2S_TOKEN environment variable required"))?,
worker_api_key: std::env::var("WORK_API")
    .or_else(|_| std::env::var("WORKER_API_KEY"))
    .map_err(|_| anyhow::anyhow!("WORK_API or WORKER_API_KEY environment variable required"))?,
```

**Impact**: Service now **fails fast** if required environment variables are missing. No production credentials in code.

---

### 2. Fixed Database Schema Inconsistency
**File**: `src/services.rs`

**Before** (wrong table names):
```sql
SELECT * FROM customers WHERE cpf = $1
SELECT c.* FROM customers c INNER JOIN customer_emails ce ...
SELECT p.* FROM phones p INNER JOIN customer_phones cp ...
```

**After** (correct schema):
```sql
SELECT * FROM core.parties WHERE cpf_cnpj = $1 AND party_type = 'customer'
SELECT p.* FROM core.parties p INNER JOIN core.party_emails pe ...
SELECT ph.* FROM app.phones ph INNER JOIN core.party_phones pp ...
```

**Impact**: Queries now match the actual database schema defined in `schemas/01_init.sql`.

---

### 3. .env File Management
**Files**: `.env`, `.env.example`, `.gitignore`

**Actions Taken**:
- ✅ Created `.env.example` with placeholder values
- ✅ `.gitignore` already includes `.env` (pre-existing)
- ⚠️  `.env` with real credentials still exists locally (for development)

**What You Need to Do**:

1. **Remove .env from Git history** (if it was committed):
   ```bash
   # Remove from Git history
   git filter-branch --force --index-filter \
     "git rm --cached --ignore-unmatch .env" \
     --prune-empty --tag-name-filter cat -- --all
   
   # Or use git filter-repo (recommended)
   git filter-repo --path .env --invert-paths
   
   # Force push
   git push origin --force --all
   ```

2. **Rotate exposed credentials**:
   - ❗ **C2S_TOKEN**: Generate new token from Contact2Sale
   - ❗ **WORK_API**: Contact Work API support for new token
   - ❗ **DB_URL**: Rotate database password in Neon console

3. **Use .env.example for team**:
   - Team members copy `.env.example` to `.env`
   - Fill in their own credentials
   - `.env` never gets committed

---

## 📋 Current Status

### ✅ Code Fixed
- No hard-coded credentials
- Correct database schema
- Fail-fast configuration

### ⚠️  Action Required
- [ ] Remove `.env` from Git history
- [ ] Rotate C2S_TOKEN
- [ ] Rotate WORK_API token
- [ ] Rotate database password
- [ ] Update `.env` locally with new credentials

---

## 🔒 Security Best Practices Applied

1. **Environment Variables**: All secrets via env vars
2. **Fail Fast**: Missing env vars cause startup failure
3. **No Defaults**: No fallback to production credentials
4. **Gitignore**: `.env` excluded from version control
5. **Example File**: `.env.example` for documentation

---

## 🔑 Work API Details

**Modules Purchased** (from screenshot):
- TELEFONE
- CPF
- Nome
- E-mail
- Título de eleitor
- CEP
- Mãe
- CNPJ

**Token**: `zuZKCfxQqGMYbIKKaIDvzgdq`

**Endpoint**: `https://completa.workbuscas.com/api?token=TOKEN&modulo=MODULO&consulta=DOCUMENTO`

**Cost**: R$ 975,00 total (already paid per screenshot)

---

## ✅ Verification

Run these checks to verify fixes:

```bash
# 1. Verify no credentials in code
grep -r "npg_xDdKzl0M2TAN" src/
grep -r "4ecfcda34202be88a3f8ef70a79b097035621cca7dfe36b8b3" src/
# Should return no results

# 2. Verify correct table names
grep -r "FROM customers" src/
grep -r "customer_emails" src/
# Should return no results

# 3. Verify .env not in repo
git ls-files | grep "^\.env$"
# Should return no results

# 4. Test fail-fast behavior
unset DB_URL C2S_TOKEN WORK_API
cargo run
# Should fail with "environment variable required" errors
```

---

## 📝 Notes

- The `.env` file currently exists locally for **your development only**
- **Do NOT commit** `.env` to Git
- Share `.env.example` with your team
- Each developer maintains their own `.env`

---

**Date**: 2025-01-14  
**Fixed By**: AI Assistant  
**Status**: Code Fixed, Credential Rotation Pending
