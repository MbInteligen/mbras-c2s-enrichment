# Security Checklist

## ⚠️ URGENT: Credentials Exposed in Development

The following credentials were present in the `.env` file during development and should be rotated:

### 1. Contact2Sale API Token
- **Status**: ⚠️ NEEDS ROTATION
- **Current**: `4ecfcda34202be88a3f8ef70a79b097035621cca7dfe36b8b3`
- **Action Required**: 
  - Log into Contact2Sale admin panel
  - Revoke/regenerate API token
  - Update production environment variables

### 2. Work API Key
- **Status**: ⚠️ NEEDS ROTATION
- **Current**: `zuZKCfxQqGMYbIKKaIDvzgdq`
- **Action Required**:
  - Contact Work API provider
  - Request new API key
  - Update production environment variables

### 3. Database Credentials (Neon)
- **Status**: ⚠️ NEEDS ROTATION
- **Current**: `postgresql://neondb_owner:npg_xDdKzl0M2TAN@ep-lively-night-ac5stqsn-pooler.sa-east-1.aws.neon.tech/neondb`
- **Action Required**:
  - Log into Neon console
  - Reset database password
  - Update production environment variables

### 4. Diretrix API Credentials
- **Status**: ⚠️ NEEDS ROTATION
- **Current User**: `100198`
- **Current Pass**: `Mb082025`
- **Action Required**:
  - Contact Diretrix support or log into admin panel
  - Change password
  - Update production environment variables

## Configuration Management

### Current State
- ✅ `.env` is in `.gitignore`
- ✅ `.env.example` created with placeholder values
- ❌ Repository not yet initialized (no git commits)
- ⚠️ Config validation needs improvement (see below)

### Recommended Actions

1. **Before initializing git repository:**
   - [ ] Rotate all credentials listed above
   - [ ] Update `.env` with new credentials
   - [ ] Verify `.env` is in `.gitignore`
   - [ ] Double-check no credentials in source code

2. **Improve config validation:**
   - [ ] Update `src/config.rs` to validate all required env vars
   - [ ] Fail fast on startup if any required var is missing
   - [ ] Add descriptive error messages for missing vars

3. **Production deployment:**
   - [ ] Use environment variables (not `.env` file) in production
   - [ ] Store secrets in secure vault (AWS Secrets Manager, etc.)
   - [ ] Never commit `.env` to version control

## Best Practices Going Forward

1. **Use `.env.example` for documentation**
   - Keep it updated with all required variables
   - Use placeholder values only
   - Document what each variable is for

2. **Rotate credentials regularly**
   - API tokens: Every 90 days minimum
   - Passwords: Every 90 days minimum
   - Database credentials: On security incidents

3. **Principle of least privilege**
   - Use read-only database credentials where possible
   - Separate dev/staging/prod credentials
   - Limit API key scopes/permissions

4. **Monitor for leaks**
   - Use tools like `gitleaks` or `truffleHog`
   - Set up GitHub secret scanning
   - Regular security audits

## Deployment Security

When deploying to production:

```bash
# ❌ DON'T: Use .env files in production
cp .env.production /var/www/app/

# ✅ DO: Use environment variables
export C2S_TOKEN="$(vault read -field=token secret/c2s)"
export DB_URL="$(vault read -field=url secret/database)"
./rust-c2s-api
```

## Status Tracking

- **Created**: 2025-11-14
- **Last Updated**: 2025-11-14
- **Credentials Rotated**: ❌ NO (URGENT)
- **Config Validation Updated**: ❌ NO
- **Production Deployment**: ❌ NOT YET

---

**Next Immediate Action**: Rotate all credentials listed above before pushing code to any repository or deploying to production.
