# Google Ads Lead Form Integration

**Purpose**: Receive Google Ads leads via webhook, enrich with Diretrix + Work API, and create fully enriched leads in Contact2Sale (C2S) CRM.

**Date**: 2025-01-21  
**Status**: ✅ Implemented

---

## 📋 Table of Contents

- [Overview](#overview)
- [Architecture](#architecture)
- [Setup Guide](#setup-guide)
- [Configuration](#configuration)
- [Testing](#testing)
- [Monitoring](#monitoring)
- [Troubleshooting](#troubleshooting)

---

## 🎯 Overview

### What This Does

1. **Receives** Google Ads Lead Form webhooks at `/api/v1/webhooks/google-ads`
2. **Validates** webhook authenticity using `google_key` parameter
3. **Extracts** contact information (name, phone, email, CPF if provided)
4. **Enriches** lead data inline:
   - If CPF provided in form → use directly
   - If no CPF → try Diretrix lookup by phone/email
   - If CPF found → enrich with Work API (income, score, addresses, etc.)
5. **Creates** lead in C2S with complete enrichment in single API call
6. **Tracks** all leads in `google_ads_leads` table with enrichment status

### Key Features

- ✅ **Single API Call**: All enrichment included in C2S lead creation (no webhook loop)
- ✅ **Idempotency**: Duplicate leads prevented via unique constraint on `google_lead_id`
- ✅ **Fallback Handling**: Lead created even if enrichment fails (with warning)
- ✅ **Validation**: Phone (E.164) and email (RFC 5322) validation before enrichment
- ✅ **Security**: Mandatory `google_key` validation for webhook authenticity
- ✅ **Performance Tracking**: C2S API latency and description length logged

---

## 🏗️ Architecture

### Flow Diagram

```
Google Ads Lead Form
        ↓
  (webhook POST)
        ↓
Rust API: /api/v1/webhooks/google-ads?google_key=XXX
        ↓
1. Validate google_key ✓
2. Check deduplication (google_lead_id unique) ✓
3. Extract & validate contact info ✓
        ↓
4. Inline Enrichment:
   ├─ CPF from form? → Use it
   ├─ No CPF? → Diretrix lookup (phone/email)
   └─ CPF found? → Work API enrichment
        ↓
5. Format complete description:
   ├─ Google Ads context (campaign, form, gclid)
   └─ Enrichment data (income, score, addresses)
        ↓
6. Create lead in C2S (via Gateway)
   ├─ customer: "João Silva"
   ├─ phone: "+5511987654321"
   ├─ email: "joao@example.com"
   ├─ description: [complete enrichment]
   └─ source: "Google Ads"
        ↓
7. Store tracking record in google_ads_leads
        ↓
   ✅ Done!
```

### Data Flow

**Input** (Google Ads webhook payload):
```json
{
  "lead_id": "abc123",
  "api_version": "v1",
  "form_id": 123456,
  "campaign_id": 789012,
  "gcl_id": "gclid-xyz",
  "google_key": "verification_key",
  "is_test": false,
  "user_column_data": [
    {"column_id": "FULL_NAME", "column_name": "Nome Completo", "string_value": "João Silva"},
    {"column_id": "EMAIL", "column_name": "E-mail", "string_value": "joao@example.com"},
    {"column_id": "PHONE_NUMBER", "column_name": "Telefone", "string_value": "11987654321"}
  ]
}
```

**Output** (C2S lead with enrichment):
```json
{
  "customer": "João Silva",
  "phone": "+5511987654321",
  "email": "joao@example.com",
  "source": "Google Ads",
  "seller_id": "508e51649fabb3502e98a32b4c6763e9",
  "description": "🎯 Lead do Google Ads\n\n📊 Campanha ID: 789012\n📝 Formulário ID: 123456\n🔗 GCLID: gclid-xyz\n\n📋 Informações do Formulário:\n   • Nome Completo: João Silva\n   • E-mail: joao@example.com\n   • Telefone: 11987654321\n\n💰 Dados Econômicos:\n   • Renda Estimada: R$ 5.000,00\n   • Score: 650\n\n🏠 Endereços:\n   1. Rua Exemplo, 123 - Centro, São Paulo/SP (CEP: 01234-567)\n..."
}
```

---

## 🚀 Setup Guide

### 1. Database Migration

Apply the Google Ads leads tracking table:

```bash
psql $DB_URL -f migrations/003_google_ads_leads.sql
```

**Verify migration**:
```sql
\d google_ads_leads
SELECT count(*) FROM google_ads_leads;
```

### 2. Environment Configuration

Add to `.env`:

```bash
# Google Ads Integration
GOOGLE_ADS_WEBHOOK_KEY=your_webhook_verification_key_here
C2S_DEFAULT_SELLER_ID=508e51649fabb3502e98a32b4c6763e9
C2S_DESCRIPTION_MAX_LENGTH=5000
```

**Generate webhook key**:
```bash
openssl rand -hex 32
# Output: a29d031c3ce8309a1e33f3846b3ff5afa34b29e6d287f5236a7a76932932eddc
```

**Get seller ID** from C2S dashboard or API.

### 3. Deploy Secrets (Fly.io)

```bash
fly secrets set \
  GOOGLE_ADS_WEBHOOK_KEY="a29d031c3ce8309a1e33f3846b3ff5afa34b29e6d287f5236a7a76932932eddc" \
  C2S_DEFAULT_SELLER_ID="508e51649fabb3502e98a32b4c6763e9" \
  C2S_DESCRIPTION_MAX_LENGTH="5000"
```

### 4. Configure Google Ads Lead Form Extension

**In Google Ads Console**:

1. Navigate to **Lead Form Extension** settings
2. Enable **Webhook delivery**
3. Set webhook URL:
   ```
   https://mbras-c2s.fly.dev/api/v1/webhooks/google-ads?google_key=a29d031c3ce8309a1e33f3846b3ff5afa34b29e6d287f5236a7a76932932eddc
   ```
4. Test webhook with **Send test lead** button
5. Verify lead appears in C2S and `google_ads_leads` table

**Documentation**: https://developers.google.com/google-ads/api/docs/leads/webhooks

---

## ⚙️ Configuration

### Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `GOOGLE_ADS_WEBHOOK_KEY` | ✅ Yes* | - | Webhook verification key (treated as mandatory at runtime) |
| `C2S_DEFAULT_SELLER_ID` | ⚠️ Recommended | - | Default seller for new leads (falls back to none if not set) |
| `C2S_DESCRIPTION_MAX_LENGTH` | ❌ No | 5000 | Max description length (truncates if exceeded) |

*\*While optional in config, the handler will reject requests without this key*

### C2S Gateway

**Required**: `C2S_GATEWAY_URL` must be set and gateway must be deployed.

**Gateway URL**: `https://mbras-c2s-gateway.fly.dev`

The integration uses the C2S Gateway's `POST /leads` endpoint for lead creation.

---

## 🧪 Testing

### Local Testing

1. **Start server**:
   ```bash
   cargo run
   ```

2. **Run test script**:
   ```bash
   ./scripts/test_google_webhook.sh http://localhost:8080 your_google_key
   ```

3. **Manual test**:
   ```bash
   curl -X POST "http://localhost:8080/api/v1/webhooks/google-ads?google_key=test_key" \
     -H "Content-Type: application/json" \
     -d '{
       "lead_id": "test-123",
       "api_version": "v1",
       "form_id": 123456,
       "campaign_id": 789012,
       "google_key": "test_key",
       "is_test": true,
       "user_column_data": [
         {"column_id": "FULL_NAME", "column_name": "Nome", "string_value": "Test User"},
         {"column_id": "EMAIL", "column_name": "E-mail", "string_value": "test@example.com"},
         {"column_id": "PHONE_NUMBER", "column_name": "Telefone", "string_value": "11987654321"}
       ]
     }'
   ```

### Production Testing

```bash
./scripts/test_google_webhook.sh https://mbras-c2s.fly.dev your_production_key
```

### Database Verification

```sql
-- Check recent Google Ads leads
SELECT 
  google_lead_id,
  c2s_lead_id,
  campaign_id,
  enrichment_status,
  cpf,
  c2s_latency_ms,
  created_at
FROM google_ads_leads
ORDER BY created_at DESC
LIMIT 10;

-- Check enrichment success rate
SELECT 
  enrichment_status,
  COUNT(*) as count,
  AVG(c2s_latency_ms) as avg_latency_ms
FROM google_ads_leads
GROUP BY enrichment_status;
```

---

## 📊 Monitoring

### Key Metrics

**Database queries**:

```sql
-- Leads processed today
SELECT COUNT(*) FROM google_ads_leads 
WHERE created_at >= CURRENT_DATE;

-- Enrichment success rate
SELECT 
  enrichment_status,
  COUNT(*),
  ROUND(100.0 * COUNT(*) / SUM(COUNT(*)) OVER (), 2) as percentage
FROM google_ads_leads
GROUP BY enrichment_status;

-- Average C2S API latency
SELECT AVG(c2s_latency_ms) as avg_latency_ms 
FROM google_ads_leads 
WHERE c2s_latency_ms IS NOT NULL;

-- Failed enrichments (investigate)
SELECT google_lead_id, error_message, payload_raw
FROM google_ads_leads
WHERE enrichment_status = 'failed'
ORDER BY created_at DESC;
```

### Logs

**Search for Google Ads webhook activity**:
```bash
fly logs -a mbras-c2s | grep "Google Ads"
```

**Look for**:
- `📨 Received Google Ads webhook` - Incoming webhooks
- `✅ Lead created in C2S` - Successful lead creation
- `⚠️  Enrichment failed` - Enrichment errors (lead still created)
- `❌ Invalid Google Ads webhook key` - Authentication failures

---

## 🔧 Troubleshooting

### Common Issues

#### 1. "Invalid google_key parameter" (401)

**Cause**: `google_key` query parameter doesn't match `GOOGLE_ADS_WEBHOOK_KEY`.

**Fix**:
```bash
# Check current key
fly secrets list -a mbras-c2s | grep GOOGLE_ADS_WEBHOOK_KEY

# Update if needed
fly secrets set GOOGLE_ADS_WEBHOOK_KEY="new_key_here"

# Update Google Ads webhook URL with new key
```

#### 2. "GOOGLE_ADS_WEBHOOK_KEY not configured" (500)

**Cause**: Environment variable not set.

**Fix**:
```bash
fly secrets set GOOGLE_ADS_WEBHOOK_KEY="your_key_here"
fly deploy  # Restart to pick up new secret
```

#### 3. Duplicate leads not detected

**Cause**: Database constraint might not be applied.

**Fix**:
```sql
-- Check constraint exists
SELECT conname FROM pg_constraint 
WHERE conname = 'uq_google_lead_id';

-- If missing, reapply migration
\i migrations/003_google_ads_leads.sql
```

#### 4. Enrichment always fails

**Cause**: Diretrix or Work API credentials/configuration issue.

**Debug**:
```bash
# Check Diretrix config
fly logs | grep "Diretrix"

# Check Work API config
fly logs | grep "Work API"

# Verify credentials
fly secrets list | grep -E "DIRETRIX|WORK_API"
```

#### 5. C2S lead creation fails

**Cause**: C2S Gateway down or misconfigured.

**Debug**:
```bash
# Check gateway URL
fly secrets list | grep C2S_GATEWAY_URL

# Test gateway health
curl https://mbras-c2s-gateway.fly.dev/

# Check gateway logs
fly logs -a mbras-c2s-gateway
```

---

## 📈 Performance

### Benchmarks

| Metric | Target | Typical |
|--------|--------|---------|
| Total webhook processing | < 5s | 2-4s |
| Diretrix lookup | < 1s | 500-800ms |
| Work API enrichment | < 3s | 1-2s |
| C2S lead creation | < 2s | 500-1500ms |
| Database insert | < 100ms | 20-50ms |

### Optimization Tips

1. **Description Length**: Keep under 5000 chars to avoid truncation
2. **Concurrent Requests**: Handled via async Rust (no issue with high volume)
3. **Rate Limiting**: No rate limiting on our end (Google Ads controls webhook frequency)

---

## 🔐 Security

### Webhook Authentication

- **Required**: `google_key` query parameter must match `GOOGLE_ADS_WEBHOOK_KEY`
- **Fail-fast**: Invalid key = immediate 401 rejection
- **No fallback**: Unlike C2S webhooks, Google Ads webhooks REQUIRE authentication

### Best Practices

1. **Rotate keys** periodically (quarterly recommended)
2. **Use long keys** (32+ bytes, generated via `openssl rand -hex 32`)
3. **Keep keys secret** (never commit to git, use Fly.io secrets)
4. **Monitor failed auth** attempts:
   ```bash
   fly logs | grep "Invalid Google Ads webhook key"
   ```

---

## 📚 References

- **Google Ads Lead Form Webhooks**: https://developers.google.com/google-ads/api/docs/leads/webhooks
- **C2S API Documentation**: Internal (via Python gateway)
- **Work API Documentation**: Contact Work API support
- **Diretrix API**: http://api.diretrixconsultoria.com.br

---

## 🎯 Next Steps

### Phase 2: Python Gateway for Remarketing (Future)

Once lead quality assessment is implemented:

1. **Build Python service** using `google-ads-python` library
2. **Add endpoint** to receive lead quality from Rust API
3. **Manage Audience Lists**:
   - High-quality leads → Add to remarketing audiences
   - Low-quality leads → Add to exclusion list
4. **Optimize campaigns** based on conversion data

**Status**: Planned (not yet implemented)

---

**Last Updated**: 2025-01-21  
**Maintained By**: MBRAS Development Team
