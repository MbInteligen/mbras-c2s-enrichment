# C2S Webhook System - Deployment Status

**Date**: 2025-11-21  
**Version**: 15  
**Status**: ✅ Production Ready  
**URL**: https://mbras-c2s.fly.dev

---

## 🎯 Complete Deployment Summary

### ✅ Webhooks Subscribed (All 3)

| Webhook | Status | Action |
|---------|--------|--------|
| `on_create_lead` | ✅ Active | Automatic enrichment (Diretrix → Work API → C2S) |
| `on_update_lead` | ✅ Active | Re-enrichment when contact info changes |
| `on_close_lead` | ✅ Active | Received (manager notification planned) |

**Subscription Confirmed**: All returned `{"success":true,"message":"Subscribed successfully"}`

---

### ✅ Features Deployed

#### 1. Email Validation
- ✅ Fake pattern detection: `999999`, `111111`, `000000`, `123456789`
- ✅ RFC 5322 format validation
- ✅ Invalid emails skipped from Diretrix lookup
- ✅ Tested with `1199999999333@gmail.com` (correctly detected as fake)

#### 2. Phone Validation
- ✅ Brazilian phone number validation (phonenumber crate)
- ✅ E.164 normalization (`11987654321` → `+5511987654321`)
- ✅ Invalid phones skipped from Diretrix lookup

#### 3. Same Person Logic Fix
- ✅ Fixed bug: Single-source CPF no longer shows incorrect "same person"
- ✅ Now correctly shows "same person" only when phone AND email match same CPF

#### 4. Webhook Infrastructure
- ✅ Database: `webhook_events` table with idempotency
- ✅ Authentication: `X-Webhook-Token` header validation
- ✅ Background processing: Tokio spawn for non-blocking enrichment
- ✅ Error handling: Failed enrichments logged with error messages

---

## 📊 System Architecture

```
C2S Platform
    ↓ (webhook: on_create_lead, on_update_lead, on_close_lead)
mbras-c2s.fly.dev/api/v1/webhooks/c2s
    ↓
webhook_events table (PostgreSQL)
    ↓ (idempotency check)
Background Job (Tokio)
    ↓
Email/Phone Validation
    ↓ (skip if invalid)
Diretrix API (CPF lookup)
    ↓
Work API (enrichment)
    ↓
C2S API (send message)
    ↓
core.parties table (storage)
```

**Processing Time**: 5-15 seconds per lead

---

## 🔧 Configuration

### Environment Variables (Fly.io Secrets)
```bash
DB_URL=postgresql://...                    # Neon PostgreSQL
WORK_API=<api_key>                         # Work API enrichment
C2S_TOKEN=4ecfcda...                       # C2S authentication
DIRETRIX_BASE_URL=http://...               # Diretrix CPF lookup
DIRETRIX_USER=100198
DIRETRIX_PASS=<password>
WEBHOOK_SECRET=a29d031c...                 # Webhook authentication
PORT=8080
```

### C2S Webhook Configuration
```json
{
  "hook_url": "https://mbras-c2s.fly.dev/api/v1/webhooks/c2s",
  "hook_action": ["on_create_lead", "on_update_lead", "on_close_lead"],
  "headers": {
    "X-Webhook-Token": "a29d031c3ce8309a1e33f3846b3ff5afa34b29e6d287f5236a7a76932932eddc"
  }
}
```

---

## 📈 Current Behavior

### on_create_lead
1. C2S sends webhook when new lead created
2. System validates email/phone
3. Diretrix finds CPF from valid contact info
4. Work API enriches CPF data
5. Enriched message sent back to C2S lead
6. Data stored in PostgreSQL

### on_update_lead
1. C2S sends webhook when lead updated
2. Idempotency check (skip if already processed)
3. Re-enrichment only if contact info changed
4. Updated message sent to C2S

### on_close_lead
1. C2S sends webhook when lead closed
2. Webhook stored in `webhook_events` table
3. **Current**: No special action
4. **Future**: Manager notification (see `PLAN_ON_CLOSE_LEAD.md`)

---

## 📋 Future Plans

### Phase 1: Manager Notifications (Near Future)
**Goal**: Notify realtor manager when leads close

**Features**:
- Database: `manager_notifications`, `realtor_managers` tables
- Email notifications to managers
- Rich HTML templates with lead details
- Manager dashboard (web UI)

**Timeline**: 2-3 weeks for MVP

**Documentation**: `docs/PLAN_ON_CLOSE_LEAD.md`

### Phase 2: Enhanced Features
- Multi-channel notifications (SMS, WhatsApp)
- Analytics dashboard
- Weekly/monthly reports
- AI-powered insights

**Timeline**: 2-3 months

---

## 🧪 Testing

### Manual Test (Success)
```bash
curl -X POST https://mbras-c2s.fly.dev/api/v1/webhooks/c2s \
  -H "Content-Type: application/json" \
  -H "X-Webhook-Token: a29d031c..." \
  -d '{
    "id": "test-fake-email-validation-002",
    "hook_action": "on_create_lead",
    "attributes": {
      "updated_at": "2025-11-21T04:35:00Z",
      "customer": {
        "name": "Maria Test",
        "email": "1199999999333@gmail.com",
        "phone": "11987654321"
      }
    }
  }'
```

**Result**: ✅ Fake email detected and skipped  
**Database**: Lead marked as failed with "Could not find CPF via Diretrix"

### Production Test
- ✅ All 3 webhooks subscribed
- ✅ Deployment successful (version 15)
- ✅ Validation working (fake emails skipped)
- ✅ Same person logic fixed

---

## 📊 Monitoring

### Check Webhook Activity
```sql
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
```

### Check Success Rate
```sql
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
```

### Check Logs
```bash
fly logs -a mbras-c2s
```

Look for:
- `"Received C2S webhook"`
- `"Skipping invalid/fake email"`
- `"✓ Phone and email belong to the same person"`
- `"Successfully sent message to C2S"`

---

## 🚨 Known Issues

### None Currently

All known issues have been resolved:
- ✅ Duplicate message bug fixed (lead-level deduplication)
- ✅ Same person logic bug fixed
- ✅ Fake email handling implemented

---

## 📚 Documentation

| Document | Purpose |
|----------|---------|
| `WEBHOOK_SUBSCRIPTION_STATUS.md` | Current subscription status |
| `C2S_WEBHOOK_CONFIGURATION.md` | How to configure C2S webhooks |
| `VALIDATION_DEPLOYMENT.md` | Email/phone validation details |
| `PLAN_ON_CLOSE_LEAD.md` | Future manager notification plan |
| `DEPLOYMENT_STATUS.md` | This document (overall status) |

---

## ✅ Deployment Checklist

- [x] Database migration applied (webhook_events table)
- [x] Webhook secret generated and configured
- [x] Code deployed to Fly.io (version 15)
- [x] Email validation implemented
- [x] Phone validation implemented
- [x] Same person logic fixed
- [x] All 3 webhooks subscribed (on_create_lead, on_update_lead, on_close_lead)
- [x] Manual testing completed
- [x] Documentation updated
- [ ] 24-48 hour monitoring period
- [ ] Manager notification implementation (planned)

---

## 🎯 Success Metrics

### Expected Performance
- **Success Rate**: >80% (some leads won't have CPF in Diretrix)
- **Processing Time**: 5-15 seconds average
- **Validation Rate**: ~10% emails detected as fake (estimated)
- **Uptime**: >99.9% (Fly.io auto-restart)

### Monitor For
- ✅ Webhooks being received
- ✅ Fake emails being detected and logged
- ✅ Enriched messages appearing in C2S leads
- ✅ No authentication errors (401)
- ✅ Reasonable failure rate for "no CPF found"

---

## 🔄 Rollback Plan

If issues arise:

### Quick Fix (Disable Validation)
```rust
// In src/enrichment.rs - always proceed with lookup
Some(email_addr.to_string())  // Skip validation check
```

### Full Rollback
```bash
git revert f57b34b
fly deploy
```

### Unsubscribe Webhooks
```bash
# If needed, unsubscribe from problematic webhook
curl -X POST https://api.contact2sale.com/integration/leads/unsubscribe \
  -H "Authentication: Bearer $C2S_TOKEN" \
  -d '{"hook_action": "on_close_lead"}'
```

---

## 📞 Support

### Logs
```bash
fly logs -a mbras-c2s
```

### Database
```bash
psql "postgresql://neondb_owner:...@ep-lively-night-ac5stqsn-pooler.sa-east-1.aws.neon.tech/neondb?sslmode=require"
```

### Deployment
```bash
fly status -a mbras-c2s
fly deploy  # Deploy new version
```

---

## 📊 Summary

**Status**: ✅ **PRODUCTION READY**

All 3 C2S webhooks are subscribed and active:
- ✅ `on_create_lead` - Automatic enrichment
- ✅ `on_update_lead` - Re-enrichment on updates
- ✅ `on_close_lead` - Stored (manager notification planned)

Email/phone validation deployed and tested. Same person logic bug fixed. System is processing webhooks in production.

**Next Steps**:
1. Monitor for 24-48 hours
2. Review webhook statistics
3. Implement manager notification (Phase 1)

---

**Last Updated**: 2025-11-21T04:55:00Z  
**Deployed By**: Engineering Team  
**Production URL**: https://mbras-c2s.fly.dev  
**Status**: 🟢 Live
