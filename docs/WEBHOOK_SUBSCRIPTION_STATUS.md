# C2S Webhook Subscription Status

**Date**: 2025-11-21  
**Status**: ✅ Active  

---

## Subscribed Webhooks

### 1. on_create_lead ✅
**Subscribed**: 2025-11-21T04:45:00Z  
**Endpoint**: https://mbras-c2s.fly.dev/api/v1/webhooks/c2s  
**Response**: `{"success":true,"message":"Subscribed successfully"}`

**Trigger**: When a new lead is created in C2S  
**Action**: Automatically enriches lead with CPF data via Diretrix → Work API → Send message back to C2S

---

### 2. on_update_lead ✅
**Subscribed**: 2025-11-21T04:45:30Z  
**Endpoint**: https://mbras-c2s.fly.dev/api/v1/webhooks/c2s  
**Response**: `{"success":true,"message":"Subscribed successfully"}`

**Trigger**: When a lead is updated in C2S  
**Action**: Re-enriches lead if customer contact info (phone/email) changed

---

### 3. on_close_lead ✅
**Subscribed**: 2025-11-21T04:50:00Z  
**Endpoint**: https://mbras-c2s.fly.dev/api/v1/webhooks/c2s  
**Response**: `{"success":true,"message":"Subscribed successfully"}`

**Trigger**: When a lead is closed in C2S (won, lost, or cancelled)  
**Current Action**: Webhook received and stored (no enrichment)  
**Future Action**: Send notification to realtor manager (see `PLAN_ON_CLOSE_LEAD.md`)

**Note**: Manager notification feature planned for near future

---

## Webhook Configuration

**Webhook URL**: https://mbras-c2s.fly.dev/api/v1/webhooks/c2s  
**Authentication**: X-Webhook-Token header (validated server-side)  
**Secret**: `a29d031c3ce8309a1e33f3846b3ff5afa34b29e6d287f5236a7a76932932eddc`  
**Method**: POST  
**Content-Type**: application/json

---

## Expected Behavior

### New Lead Flow (`on_create_lead`)

1. **C2S creates lead** → Sends webhook to our API
2. **Our API receives webhook** → Stores in `webhook_events` table
3. **Background job starts** → Enriches lead data
4. **Diretrix lookup** → Find CPF from phone/email
5. **Work API enrichment** → Get complete person data
6. **Send message to C2S** → Display enriched data in lead
7. **Store in database** → Save to `core.parties` table

**Duration**: 5-15 seconds (average)

### Updated Lead Flow (`on_update_lead`)

1. **C2S updates lead** → Sends webhook if customer data changed
2. **Idempotency check** → Skip if same `lead_id` + `updated_at` already processed
3. **Re-enrichment** → Only if phone/email changed
4. **Update C2S** → Send new enriched message

---

## Monitoring

### Check Webhook Activity

```bash
# View recent webhooks
psql "$DB_URL" -c "
  SELECT 
    lead_id,
    hook_action,
    status,
    received_at,
    processed_at,
    error_message
  FROM webhook_events
  ORDER BY received_at DESC
  LIMIT 10;
"
```

### Check Success Rate

```bash
# Daily webhook statistics
psql "$DB_URL" -c "
  SELECT 
    DATE(received_at) as date,
    hook_action,
    COUNT(*) as total,
    COUNT(*) FILTER (WHERE status = 'completed') as completed,
    COUNT(*) FILTER (WHERE status = 'failed') as failed,
    ROUND(100.0 * COUNT(*) FILTER (WHERE status = 'completed') / COUNT(*), 1) as success_rate
  FROM webhook_events
  WHERE received_at > NOW() - INTERVAL '7 days'
  GROUP BY DATE(received_at), hook_action
  ORDER BY date DESC, hook_action;
"
```

---

## Testing

### Test Webhook Manually

```bash
curl -X POST https://mbras-c2s.fly.dev/api/v1/webhooks/c2s \
  -H "Content-Type: application/json" \
  -H "X-Webhook-Token: a29d031c3ce8309a1e33f3846b3ff5afa34b29e6d287f5236a7a76932932eddc" \
  -d '{
    "id": "test-webhook-001",
    "hook_action": "on_create_lead",
    "attributes": {
      "updated_at": "2025-11-21T05:00:00Z",
      "customer": {
        "name": "Test User",
        "email": "test@example.com",
        "phone": "11987654321"
      }
    }
  }'
```

**Expected**: `{"status":"received","received":1,"processed":1,"duplicates":0}`

### Test with Real Lead

1. Create a new lead in C2S dashboard
2. Fill in customer name, phone, and email
3. Save lead
4. Wait 5-15 seconds
5. Check lead messages in C2S (should see enriched data)

---

## Validation Features (Deployed)

### Email Validation ✅
- Detects fake patterns: `999999`, `111111`, `000000`, `123456789`
- RFC 5322 format validation
- Skips invalid emails from Diretrix lookup

**Example**: Email `1199999999333@gmail.com` is detected as fake and skipped

### Phone Validation ✅
- Brazilian phone number validation
- E.164 normalization (`11987654321` → `+5511987654321`)
- Skips invalid phones from Diretrix lookup

### Same Person Logic Fix ✅
- Fixed bug where single-source CPF incorrectly showed "same person"
- Now correctly shows "same person" only when phone AND email return same CPF

---

## Troubleshooting

### No Webhooks Received

**Check**:
1. Verify subscriptions are active (see above)
2. Check C2S dashboard webhook logs (if available)
3. Verify webhook URL is correct in C2S settings
4. Check Fly.io app is running: `fly status -a mbras-c2s`

**Fix**:
```bash
# Resubscribe if needed
curl -X POST https://api.contact2sale.com/integration/leads/subscribe \
  -H "Content-Type: application/json" \
  -H "Authentication: Bearer $C2S_TOKEN" \
  -d '{"hook_url": "https://mbras-c2s.fly.dev/api/v1/webhooks/c2s", "hook_action": "on_create_lead"}'
```

### Webhook Received but Failed

**Check error message**:
```sql
SELECT lead_id, error_message 
FROM webhook_events 
WHERE status = 'failed'
ORDER BY received_at DESC
LIMIT 10;
```

**Common errors**:
- `"Could not find CPF via Diretrix"` → Expected (phone/email not in database)
- `"Invalid email detected"` → Fake email skipped (working as intended)
- `"Lead not found"` → Test lead doesn't exist in C2S

### Duplicate Processing

**Idempotency** ensures same lead+timestamp is only processed once:
- Unique constraint: `(lead_id, updated_at)`
- Duplicate webhooks return: `{"duplicates": 1, "processed": 0}`

---

## Unsubscribe (If Needed)

If you need to unsubscribe from webhooks:

```bash
# Check C2S documentation for unsubscribe endpoint
# Typically: DELETE or POST with action=unsubscribe
curl -X POST https://api.contact2sale.com/integration/leads/unsubscribe \
  -H "Content-Type: application/json" \
  -H "Authentication: Bearer $C2S_TOKEN" \
  -d '{"hook_action": "on_create_lead"}'
```

**Note**: Confirm exact endpoint with C2S API documentation

---

## Summary

✅ **Active Subscriptions**:
- `on_create_lead` → New leads automatically enriched
- `on_update_lead` → Updated leads re-enriched when contact info changes
- `on_close_lead` → Webhook received (manager notification planned for near future)

✅ **Validation Active**:
- Fake email detection preventing wasted API calls
- Phone validation with E.164 normalization
- Same person logic fixed

✅ **Monitoring**:
- `webhook_events` table tracks all webhooks
- Idempotency prevents duplicate processing
- Error messages logged for debugging

✅ **Next Steps**:
1. Monitor webhook activity for 24-48 hours
2. Check success rate (expect >80% completion)
3. Review failed enrichments (some failures expected for contacts without CPF)
4. Verify enriched messages appearing in C2S leads

---

**Last Updated**: 2025-11-21T04:45:30Z  
**Status**: ✅ Production Ready  
**Monitoring**: Active
