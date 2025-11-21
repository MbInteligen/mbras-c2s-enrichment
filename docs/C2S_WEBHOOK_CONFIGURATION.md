# C2S Webhook Configuration Guide

**Purpose**: Configure Contact2Sale webhook to send lead events to our API  
**Endpoint**: https://mbras-c2s.fly.dev/api/v1/webhooks/c2s

---

## C2S Webhook Documentation

**Official Docs**: https://api.contact2sale.com/docs/api#webhook

### Available Hook Actions

C2S supports three webhook events:

1. `on_create_lead` - Triggered when a new lead is created
2. `on_update_lead` - Triggered when a lead is updated
3. `on_close_lead` - Triggered when a lead is closed

⚠️ **Important**: 
- `hook_action` is a **required field**
- Sending wrong `hook_action` returns **422 error**
- Webhook data format is same as `GET /leads/{id}` response

---

## Configuration Steps

### Option 1: Via C2S Dashboard (Recommended)

1. **Log in to C2S Dashboard**
   - URL: https://app.contact2sale.com (or your C2S URL)
   - Use your admin credentials

2. **Navigate to Webhooks**
   - Go to **Settings** → **Integrations** → **Webhooks**
   - Or direct URL: https://app.contact2sale.com/settings/webhooks

3. **Add New Webhook**
   - Click **"Add Webhook"** or **"New Webhook"** button

4. **Configure Webhook URL**
   ```
   URL: https://mbras-c2s.fly.dev/api/v1/webhooks/c2s
   Method: POST
   ```

5. **Add Authentication Header**
   ```
   Header Name: X-Webhook-Token
   Header Value: a29d031c3ce8309a1e33f3846b3ff5afa34b29e6d287f5236a7a76932932eddc
   ```

6. **Select Events** (Hook Actions)
   - [x] `on_create_lead` - Subscribe to new leads
   - [x] `on_update_lead` - Subscribe to lead updates
   - [ ] `on_close_lead` - (Optional) Subscribe to closed leads

7. **Save and Enable**
   - Click **Save** or **Create**
   - Toggle webhook to **Enabled** state
   - Status should show **Active** or **Enabled**

---

### Option 2: Via C2S API (Advanced)

If webhook configuration must be done via API:

```bash
curl -X POST https://api.contact2sale.com/api/v1/webhooks \
  -H "Authorization: Bearer YOUR_C2S_API_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "url": "https://mbras-c2s.fly.dev/api/v1/webhooks/c2s",
    "hook_action": "on_create_lead",
    "headers": {
      "X-Webhook-Token": "a29d031c3ce8309a1e33f3846b3ff5afa34b29e6d287f5236a7a76932932eddc"
    },
    "active": true
  }'
```

Repeat for each `hook_action`:
- `on_create_lead`
- `on_update_lead`
- `on_close_lead` (optional)

**Note**: Check C2S API documentation for exact endpoint and payload format.

---

## Webhook Payload Format

### Expected Payload from C2S

Based on C2S documentation, webhook payload structure:

```json
{
  "id": "lead-12345",
  "hook_action": "on_create_lead",
  "attributes": {
    "updated_at": "2025-01-21T10:30:00Z",
    "customer": {
      "name": "João Silva",
      "email": "joao@example.com",
      "phone": "+5511987654321"
    },
    "product": {
      "description": "Apartamento 3 quartos",
      "prop_ref": "APT-001",
      "price": "R$ 500.000"
    },
    "lead_status": {
      "alias": "new",
      "name": "Novo Lead"
    },
    "log": [],
    "messages": []
  }
}
```

**Key Fields**:
- `id` - Lead ID (required for idempotency)
- `hook_action` - Event type (on_create_lead, on_update_lead, on_close_lead)
- `attributes.updated_at` - Timestamp (required for idempotency)
- `attributes.customer` - Customer data (name, email, phone)

---

## Verification

### 1. Test Webhook (Manual)

After configuration, test with curl:

```bash
curl -X POST https://mbras-c2s.fly.dev/api/v1/webhooks/c2s \
  -H "Content-Type: application/json" \
  -H "X-Webhook-Token: a29d031c3ce8309a1e33f3846b3ff5afa34b29e6d287f5236a7a76932932eddc" \
  -d '{
    "id": "test-lead-001",
    "hook_action": "on_create_lead",
    "attributes": {
      "updated_at": "2025-01-21T10:30:00Z",
      "customer": {
        "name": "Test User",
        "email": "test@example.com",
        "phone": "+5511999999999"
      }
    }
  }'
```

**Expected Response**:
```json
{
  "status": "received",
  "received": 1,
  "processed": 1,
  "duplicates": 0
}
```

### 2. Create Test Lead in C2S

1. Go to C2S Dashboard
2. Create a new lead manually
3. Fill in customer data (name, phone, email)
4. Save lead

### 3. Check Webhook Received

**Check Logs**:
```bash
fly logs -a mbras-c2s --tail
```

Look for:
```
INFO Received C2S webhook
INFO Processing 1 webhook event(s)
INFO Starting background enrichment for lead_id=...
```

**Check Database**:
```sql
SELECT lead_id, status, hook_action, received_at, processed_at
FROM webhook_events
WHERE hook_action IN ('on_create_lead', 'on_update_lead', 'on_close_lead')
ORDER BY received_at DESC
LIMIT 5;
```

**Expected**:
- New row with `hook_action = 'on_create_lead'`
- `status = 'received'` initially
- `status = 'processing'` → `completed` within 5-15 seconds

### 4. Check Enrichment Result

**Check C2S Lead**:
1. Open the lead in C2S dashboard
2. Check messages/timeline
3. Should see enriched data message with:
   - 📞📧 Same person indicator (or ⚠️ different people)
   - CPF information
   - Work API enriched data (income, score, etc.)

**Check Database**:
```sql
-- Check if person was stored
SELECT * FROM core.parties 
WHERE created_at > NOW() - INTERVAL '1 hour'
ORDER BY created_at DESC
LIMIT 5;
```

---

## Troubleshooting

### Issue: Webhook Returns 401

**Cause**: Missing or incorrect `X-Webhook-Token` header

**Fix**:
1. Check C2S webhook configuration
2. Verify header name is exactly `X-Webhook-Token`
3. Verify header value matches: `a29d031c3ce8309a1e33f3846b3ff5afa34b29e6d287f5236a7a76932932eddc`

### Issue: Webhook Returns 400

**Cause**: Missing required fields (`updated_at` or invalid timestamp)

**Check C2S Payload**:
- Ensure `attributes.updated_at` is present
- Ensure timestamp is ISO 8601 format (YYYY-MM-DDTHH:MM:SSZ)

### Issue: Webhook Returns 422 (from C2S)

**Cause**: Invalid `hook_action` value

**Fix**:
- Use only: `on_create_lead`, `on_update_lead`, or `on_close_lead`
- Check spelling and capitalization (lowercase, underscores)

### Issue: Webhook Received but Enrichment Fails

**Check Error Message**:
```sql
SELECT lead_id, error_message 
FROM webhook_events 
WHERE status = 'failed'
ORDER BY received_at DESC;
```

**Common Errors**:

1. **"Missing customer data"**
   - C2S webhook didn't include customer attributes
   - Check C2S payload structure

2. **"Could not find CPF via Diretrix"**
   - Phone/email not in Diretrix database
   - Expected for some leads (not all contacts have CPF)
   - **Action**: Lead marked as failed (expected behavior)

3. **"Lead not found"** (from C2S)
   - Trying to send message to non-existent lead
   - Only happens with test leads
   - Real leads from C2S should not have this error

### Issue: No Webhook Received

**Check C2S Configuration**:
1. Webhook URL is correct: `https://mbras-c2s.fly.dev/api/v1/webhooks/c2s`
2. Webhook is **Enabled/Active**
3. Event types are selected (`on_create_lead`, etc.)
4. Header is configured correctly

**Check Firewall**:
- Fly.io allows all incoming HTTPS traffic (no firewall issues)

**Check C2S Logs**:
- Some C2S dashboards show webhook delivery logs
- Check for failed delivery attempts or errors

---

## Recommended Configuration

### For Production Use

**Subscribe to**:
- ✅ `on_create_lead` - Essential (new leads need enrichment)
- ✅ `on_update_lead` - Recommended (capture updated contact info)
- ⚠️ `on_close_lead` - Optional (may not need enrichment for closed leads)

**Reasoning**:
- `on_create_lead`: Primary use case - enrich new leads immediately
- `on_update_lead`: Capture updated phone/email if customer info changes
- `on_close_lead`: Usually not needed (lead already processed)

### Batch vs Individual Webhooks

C2S may send:
- **Single webhook** per event (most common)
- **Batch webhooks** (array of events)

Our system handles both automatically:
```json
// Single event (object)
{"id": "lead-001", "attributes": {...}}

// Batch events (array)
[
  {"id": "lead-001", "attributes": {...}},
  {"id": "lead-002", "attributes": {...}},
  {"id": "lead-003", "attributes": {...}}
]
```

---

## Monitoring Best Practices

### Daily Checks (First Week)

```bash
# Check webhook volume
psql $DB_URL -c "
  SELECT 
    DATE(received_at) as date,
    COUNT(*) as total,
    COUNT(*) FILTER (WHERE status = 'completed') as completed,
    COUNT(*) FILTER (WHERE status = 'failed') as failed
  FROM webhook_events
  WHERE received_at > NOW() - INTERVAL '7 days'
  GROUP BY DATE(received_at)
  ORDER BY date DESC;
"
```

**Expected**:
- Most events: `status = 'completed'`
- Few events: `status = 'failed'` (expected for leads without CPF)
- Processing time: 5-15 seconds average

### Watch for Patterns

```bash
# Check common errors
psql $DB_URL -c "
  SELECT 
    error_message,
    COUNT(*) as occurrences
  FROM webhook_events
  WHERE status = 'failed'
  GROUP BY error_message
  ORDER BY occurrences DESC;
"
```

---

## Security Notes

### Webhook Secret

**Current Secret**: `a29d031c3ce8309a1e33f3846b3ff5afa34b29e6d287f5236a7a76932932eddc`

**Rotation** (if needed):
```bash
# Generate new secret
openssl rand -hex 32

# Update in Fly.io
fly secrets set WEBHOOK_SECRET="<new_secret>" -a mbras-c2s

# Update in C2S dashboard
# (Update X-Webhook-Token header with new value)
```

**Best Practice**: Rotate webhook secret quarterly or after security incidents.

---

## Summary

✅ **Required Actions**:
1. Configure webhook URL in C2S dashboard
2. Add authentication header (X-Webhook-Token)
3. Subscribe to `on_create_lead` and `on_update_lead`
4. Enable webhook
5. Test with a real lead
6. Monitor webhook_events table

✅ **Verification**:
- Create test lead in C2S
- Check webhook received (logs + database)
- Check enrichment completed (database + C2S message)
- Monitor for 24 hours

✅ **Success Criteria**:
- Webhooks received with correct `hook_action`
- Background enrichment completes
- Enriched messages appear in C2S leads
- No authentication errors (401)
- Reasonable success rate (>80% completed)

---

**Last Updated**: 2025-01-21  
**Status**: Ready for Configuration  
**Support**: See main documentation in `docs/`
