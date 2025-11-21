# C2S Webhook Manual Configuration Guide

**Issue**: The `/leads/subscribe` API subscribes to events but doesn't configure authentication headers automatically.

**Solution**: Manually configure webhook in C2S Dashboard

---

## Step-by-Step Configuration

### 1. Access C2S Dashboard

Go to: https://app.contact2sale.com (or your C2S URL)

### 2. Navigate to Webhook Settings

**Path**: Settings → Integrations → Webhooks

Or try direct URL:
- https://app.contact2sale.com/settings/webhooks
- https://app.contact2sale.com/settings/integrations

### 3. Add New Webhook

Click **"Add Webhook"** or **"New Webhook"** or **"+"** button

### 4. Configure Webhook Details

Fill in the following fields:

#### **Webhook URL**
```
https://mbras-c2s.fly.dev/api/v1/webhooks/c2s
```

#### **Method**
```
POST
```

#### **Events to Subscribe** (Select all 3)
- ✅ `on_create_lead` - When lead is created
- ✅ `on_update_lead` - When lead is updated  
- ✅ `on_close_lead` - When lead is closed

#### **Headers** (CRITICAL - Authentication)
Add a custom header:

**Header Name:**
```
X-Webhook-Token
```

**Header Value:**
```
a29d031c3ce8309a1e33f3846b3ff5afa34b29e6d287f5236a7a76932932eddc
```

⚠️ **Important**: Without this header, webhooks will be rejected with 401 Unauthorized

### 5. Enable/Activate Webhook

Make sure the webhook is:
- ✅ **Enabled** or **Active**
- ✅ Toggle switched to ON
- ✅ Status shows "Active" or "Enabled"

### 6. Save Configuration

Click **"Save"**, **"Create"**, or **"Confirm"**

---

## Verify Configuration

### Test 1: Create a New Lead

1. In C2S, create a new test lead
2. Fill in customer data:
   - Name: "Test Webhook User"
   - Phone: "11987654321"
   - Email: "test@example.com"
3. Save the lead
4. Wait 5-15 seconds

### Test 2: Check if Webhook Received

**Option A: Check Database**
```bash
psql "$DB_URL" -c "
  SELECT lead_id, hook_action, status, received_at 
  FROM webhook_events 
  WHERE received_at > NOW() - INTERVAL '5 minutes'
  ORDER BY received_at DESC;
"
```

**Option B: Check Logs**
```bash
fly logs -a mbras-c2s
```

Look for:
```
INFO Received C2S webhook
INFO Processing 1 webhook event(s)
INFO Starting background enrichment for lead_id=...
```

### Test 3: Check Lead in C2S

1. Open the test lead in C2S
2. Go to Messages or Activity/Timeline
3. Should see enriched data message within 15 seconds

**Expected Message**:
```
📞📧 Informações Enriquecidas

✓ Telefone e e-mail da mesma pessoa
CPF: XXX.XXX.XXX-XX

[Additional enriched data from Work API]
```

---

## Troubleshooting

### Issue: Webhook Still Not Received

**Check 1: Verify Header is Configured**
- Header name must be exactly: `X-Webhook-Token` (case-sensitive)
- Header value must match the secret exactly
- No extra spaces or characters

**Check 2: Verify URL is Correct**
- URL: `https://mbras-c2s.fly.dev/api/v1/webhooks/c2s`
- Must be HTTPS (not HTTP)
- No trailing slash

**Check 3: Check C2S Webhook Logs**
- Some C2S dashboards show webhook delivery logs
- Look for failed delivery attempts
- Check error messages

**Check 4: Test Manually**
```bash
curl -X POST https://mbras-c2s.fly.dev/api/v1/webhooks/c2s \
  -H "Content-Type: application/json" \
  -H "X-Webhook-Token: a29d031c3ce8309a1e33f3846b3ff5afa34b29e6d287f5236a7a76932932eddc" \
  -d '{
    "id": "manual-test-001",
    "hook_action": "on_create_lead",
    "attributes": {
      "updated_at": "2025-11-21T05:00:00Z",
      "customer": {
        "name": "Manual Test",
        "email": "manual@test.com",
        "phone": "11999999999"
      }
    }
  }'
```

Expected: `{"status":"received","received":1,"processed":1,"duplicates":0}`

---

## Alternative: Contact C2S Support

If you can't find webhook configuration in the dashboard:

1. **Contact C2S Support**
   - Ask them how to configure webhooks with custom headers
   - Provide them:
     - Webhook URL: `https://mbras-c2s.fly.dev/api/v1/webhooks/c2s`
     - Required header: `X-Webhook-Token: a29d031c...`
     - Events: on_create_lead, on_update_lead, on_close_lead

2. **Check C2S Documentation**
   - https://api.contact2sale.com/docs/api#webhook
   - Look for webhook configuration instructions
   - Some platforms require webhooks to be configured via support ticket

---

## What We've Done vs What's Needed

### ✅ Done (via API)
- Subscribed to `on_create_lead` event
- Subscribed to `on_update_lead` event
- Subscribed to `on_close_lead` event

### ❌ Missing (Manual Dashboard Configuration)
- Authentication header (`X-Webhook-Token`)
- Webhook URL in C2S system
- Webhook activation/enable toggle

**Why**: The `/leads/subscribe` API only registers event interest, but doesn't configure the webhook endpoint and authentication.

---

## Expected Dashboard Screenshot

Your C2S webhook configuration should look like:

```
┌─────────────────────────────────────────────────────────┐
│ Webhook Configuration                                   │
├─────────────────────────────────────────────────────────┤
│ URL: https://mbras-c2s.fly.dev/api/v1/webhooks/c2s     │
│ Method: POST                                            │
│                                                         │
│ Events:                                                 │
│ ✅ on_create_lead                                       │
│ ✅ on_update_lead                                       │
│ ✅ on_close_lead                                        │
│                                                         │
│ Headers:                                                │
│ X-Webhook-Token: a29d031c3ce8309a1e33f3846b3ff5afa... │
│                                                         │
│ Status: ✅ Active                                       │
│                                                         │
│ [Save] [Cancel]                                        │
└─────────────────────────────────────────────────────────┘
```

---

## Once Configured

After manual configuration in C2S dashboard:

1. **Create a test lead** in C2S
2. **Wait 5-15 seconds**
3. **Check the lead** - should see enriched message
4. **Monitor** for 24 hours to ensure it's working consistently

---

## Summary

🔴 **Current Issue**: Webhooks not being sent because:
- `/leads/subscribe` API doesn't configure authentication header
- Manual dashboard configuration required

✅ **Solution**: 
1. Go to C2S Dashboard → Settings → Webhooks
2. Add webhook URL
3. **Add authentication header** (critical!)
4. Enable webhook
5. Test with a new lead

📞 **Need Help?**: Contact C2S support if you can't find webhook settings in dashboard

---

**Created**: 2025-11-21  
**Status**: ⚠️ Manual Configuration Required
