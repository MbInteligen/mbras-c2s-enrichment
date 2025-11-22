# Google Ads Integration - Deployment Ready

**Status**: ✅ **READY FOR DEPLOYMENT**  
**Date**: 2025-01-21  
**Implementation**: Complete with security fixes applied

---

## ✅ Implementation Complete

### Core Features Implemented

1. **Google Ads Webhook Handler** (`src/google_ads_handler.rs`)
   - Receives Google Ads Lead Form webhooks
   - Validates `google_key` for security
   - Extracts and validates contact info (name, phone, email, CPF)
   - Performs inline enrichment (Diretrix → Work API)
   - Creates fully enriched leads in C2S via gateway
   - Tracks all leads in database

2. **Data Models** (`src/google_ads_models.rs`)
   - Complete webhook payload structure
   - Helper methods to extract form fields
   - UTF-8 safe description formatting

3. **Database Tracking** (`migrations/003_google_ads_leads.sql`)
   - `google_ads_leads` table with deduplication
   - Enrichment status tracking
   - Performance metrics (latency, description length)

4. **Gateway Integration** (`src/gateway_client.rs`)
   - `create_lead()` method for C2S lead creation
   - Full support for enriched descriptions

5. **Configuration** (`src/config.rs`)
   - `GOOGLE_ADS_WEBHOOK_KEY` - webhook verification
   - `C2S_DEFAULT_SELLER_ID` - default seller (optional)
   - `C2S_DESCRIPTION_MAX_LENGTH` - max chars (default: 5000)

6. **Testing & Documentation**
   - `scripts/test_google_webhook.sh` - integration tests
   - `docs/GOOGLE_ADS_INTEGRATION.md` - complete setup guide

---

## 🔒 Security Fixes Applied

### ✅ Fix 1: UTF-8 Safe String Truncation
**Issue**: Byte-level string slicing could panic on emoji/accent boundaries  
**Fix**: Changed to character-based truncation using `.chars().take(n).collect()`  
**Impact**: Prevents runtime panics when descriptions contain multibyte UTF-8 characters

**Before:**
```rust
&description[..max_desc_len]  // ❌ Can panic on UTF-8 boundaries
```

**After:**
```rust
description.chars().take(max_desc_len).collect::<String>()  // ✅ UTF-8 safe
```

### ✅ Fix 2: Secrets Protection
**Issue**: `google-ads.yaml` contained real Google Ads API credentials  
**Fix**: 
- Added `google-ads.yaml` to `.gitignore`
- Created `google-ads.yaml.example` template
- File was NOT committed to git (verified)

**Credentials Protected:**
- Developer Token
- Client ID & Secret
- OAuth2 Refresh Token
- Customer IDs

### ✅ Fix 3: Config Validation
**Issue**: `c2s_description_max_length` was optional but used without defaults  
**Fix**: Made it non-optional with hard default of 5000 chars

**Before:**
```rust
pub c2s_description_max_length: Option<usize>
// Usage: unwrap_or(5000) scattered in code
```

**After:**
```rust
pub c2s_description_max_length: usize  // Non-optional
// Set in config: .unwrap_or(5000) at initialization
```

**Additional Validation:**
- `GOOGLE_ADS_WEBHOOK_KEY` validated at request time (fails with 500 if missing)
- `C2S_DEFAULT_SELLER_ID` logged as warning if not set
- `C2S_GATEWAY_URL` checked at runtime (fails with 500 if missing)

---

## 📋 Pre-Deployment Checklist

### Database Migration

```bash
# Connect to production database
psql $DB_URL

# Apply migration
\i migrations/003_google_ads_leads.sql

# Verify table created
\d google_ads_leads

# Check constraint
SELECT conname FROM pg_constraint WHERE conname = 'uq_google_lead_id';
```

**Expected Output:**
```
CREATE TABLE
CREATE INDEX
CREATE INDEX
...
COMMENT
```

### Environment Configuration

**Required secrets** (set via Fly.io):
```bash
# Generate webhook key
WEBHOOK_KEY=$(openssl rand -hex 32)

# Set secrets
fly secrets set \
  GOOGLE_ADS_WEBHOOK_KEY="$WEBHOOK_KEY" \
  C2S_DEFAULT_SELLER_ID="508e51649fabb3502e98a32b4c6763e9" \
  C2S_DESCRIPTION_MAX_LENGTH="5000"
```

**Verify gateway is configured:**
```bash
fly secrets list | grep C2S_GATEWAY_URL
# Should show: https://mbras-c2s-gateway.fly.dev
```

### Pre-Flight Checks

- [x] Code compiles without errors ✅
- [x] UTF-8 truncation fixed ✅
- [x] Secrets protected (not in git) ✅
- [x] Config fields validated ✅
- [ ] Migration applied to production database
- [ ] Secrets configured in Fly.io
- [ ] Gateway URL verified

---

## 🚀 Deployment Steps

### Step 1: Apply Migration

```bash
# Get database URL from Fly.io
DB_URL=$(fly secrets list | grep DB_URL | awk '{print $2}')

# Apply migration
psql "$DB_URL" -f migrations/003_google_ads_leads.sql
```

**Verify:**
```sql
SELECT COUNT(*) FROM google_ads_leads;
-- Should return: 0 (empty table)
```

### Step 2: Configure Secrets

```bash
# Generate webhook key (save this!)
openssl rand -hex 32 > google_webhook_key.txt
WEBHOOK_KEY=$(cat google_webhook_key.txt)

# Set production secrets
fly secrets set \
  GOOGLE_ADS_WEBHOOK_KEY="$WEBHOOK_KEY" \
  C2S_DEFAULT_SELLER_ID="508e51649fabb3502e98a32b4c6763e9" \
  C2S_DESCRIPTION_MAX_LENGTH="5000"

# Verify secrets set
fly secrets list | grep -E "GOOGLE_ADS|C2S_DEFAULT|C2S_DESCRIPTION"
```

### Step 3: Deploy to Production

```bash
# Deploy application
fly deploy

# Monitor deployment
fly logs --tail

# Wait for: "Configuration loaded successfully"
# Wait for: "Google Ads webhook key configured"
# Wait for: "C2S description max length: 5000 chars"
```

### Step 4: Verify Deployment

```bash
# Health check
curl https://mbras-c2s.fly.dev/health

# Test webhook endpoint (should get 401 without key)
curl -X POST https://mbras-c2s.fly.dev/api/v1/webhooks/google-ads \
  -H "Content-Type: application/json" \
  -d '{"lead_id":"test","api_version":"v1","form_id":1,"campaign_id":1,"google_key":"wrong","is_test":true,"user_column_data":[{"column_id":"FULL_NAME","column_name":"Name","string_value":"Test"}]}'

# Expected: {"success":false,"message":"Invalid google_key parameter"}
```

### Step 5: Configure Google Ads

**In Google Ads Console:**

1. Navigate to **Lead Form Extension** settings
2. Enable **Webhook delivery**
3. Set webhook URL:
   ```
   https://mbras-c2s.fly.dev/api/v1/webhooks/google-ads?google_key=<YOUR_KEY_FROM_google_webhook_key.txt>
   ```
4. Test with **Send test lead** button
5. Verify in database:
   ```sql
   SELECT * FROM google_ads_leads ORDER BY created_at DESC LIMIT 1;
   ```

---

## 📊 Post-Deployment Monitoring

### Database Queries

```sql
-- Check recent leads
SELECT 
  google_lead_id,
  c2s_lead_id,
  campaign_id,
  enrichment_status,
  c2s_latency_ms,
  created_at
FROM google_ads_leads
ORDER BY created_at DESC
LIMIT 10;

-- Check enrichment success rate
SELECT 
  enrichment_status,
  COUNT(*) as count,
  ROUND(100.0 * COUNT(*) / SUM(COUNT(*)) OVER (), 2) as percentage,
  AVG(c2s_latency_ms) as avg_latency_ms
FROM google_ads_leads
GROUP BY enrichment_status;

-- Check for failures
SELECT 
  google_lead_id,
  error_message,
  created_at
FROM google_ads_leads
WHERE enrichment_status = 'failed'
ORDER BY created_at DESC;
```

### Log Monitoring

```bash
# Watch for incoming webhooks
fly logs | grep "Google Ads"

# Look for these patterns:
# ✅ "📨 Received Google Ads webhook"
# ✅ "✅ Lead created in C2S"
# ⚠️  "⚠️  Enrichment failed" (lead still created)
# ❌ "❌ Invalid Google Ads webhook key" (auth failure)
```

### Performance Metrics

**Expected Performance:**
- Total processing: 2-5 seconds
- Diretrix lookup: 500-800ms
- Work API enrichment: 1-2 seconds
- C2S lead creation: 500-1500ms

**Alert if:**
- C2S latency > 3000ms consistently
- Enrichment failure rate > 50%
- Auth failures (invalid keys)

---

## 🔧 Troubleshooting

### Issue: Webhook Returns 401 Unauthorized

**Cause**: `google_key` doesn't match `GOOGLE_ADS_WEBHOOK_KEY`

**Fix:**
```bash
# Check configured key
fly secrets list | grep GOOGLE_ADS_WEBHOOK_KEY

# Update if needed
fly secrets set GOOGLE_ADS_WEBHOOK_KEY="new_key"

# Update Google Ads webhook URL with new key
```

### Issue: Webhook Returns 500 Internal Error

**Possible Causes:**
1. Gateway not configured
2. Database connection failed
3. Missing required config

**Debug:**
```bash
# Check logs
fly logs | tail -100

# Verify gateway URL
fly secrets list | grep C2S_GATEWAY_URL

# Test gateway
curl https://mbras-c2s-gateway.fly.dev/
```

### Issue: Leads Created But Not Enriched

**Cause**: Diretrix or Work API issues

**Check:**
```bash
# Check Diretrix credentials
fly secrets list | grep DIRETRIX

# Check Work API key
fly secrets list | grep WORK_API

# Look for enrichment errors in logs
fly logs | grep "enrichment failed"
```

### Issue: Description Truncated

**Normal**: If description > 5000 chars  
**Check**: `c2s_description_max_length` in database  
**Adjust** if needed:
```bash
fly secrets set C2S_DESCRIPTION_MAX_LENGTH="10000"
```

---

## 📚 References

- **Setup Guide**: `docs/GOOGLE_ADS_INTEGRATION.md`
- **API Endpoints**: `docs/API_ENDPOINTS.md`
- **Database Schema**: `migrations/003_google_ads_leads.sql`
- **Test Script**: `scripts/test_google_webhook.sh`

---

## ✅ Deployment Sign-Off

**Pre-Deployment:**
- [x] Code review complete
- [x] Security issues fixed
- [x] Compilation successful
- [x] Documentation complete

**Deployment:**
- [ ] Migration applied (sign: _____  date: _____)
- [ ] Secrets configured (sign: _____  date: _____)
- [ ] Application deployed (sign: _____  date: _____)
- [ ] Google Ads configured (sign: _____  date: _____)
- [ ] Post-deployment tests pass (sign: _____  date: _____)

**Production Ready**: ✅ YES

---

**Last Updated**: 2025-01-21  
**Reviewed By**: AI Assistant  
**Status**: Ready for production deployment
