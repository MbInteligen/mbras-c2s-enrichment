# C2S Webhook Deployment Steps

**Quick reference for deploying the webhook implementation**

---

## Pre-Deployment Checklist

- [x] Code implemented and reviewed
- [x] Migration script created (`migrations/002_create_webhook_events.sql`)
- [x] Test script created (`scripts/test_webhook.sh`)
- [x] Documentation complete
- [ ] Database migration applied
- [ ] Webhook secret configured
- [ ] Code deployed
- [ ] Integration tests passed
- [ ] C2S webhook configured

---

## Step-by-Step Deployment

### 1. Apply Database Migration

**Connect to production database**:
```bash
# Get DB URL from Fly.io secrets
fly ssh console -a mbras-c2s
echo $DATABASE_URL

# Or use local psql
psql <production_db_url>
```

**Run migration**:
```bash
psql $DB_URL -f migrations/002_create_webhook_events.sql
```

**Verify**:
```sql
-- Should return 0 (empty table)
SELECT COUNT(*) FROM webhook_events;

-- Should show 4 indexes
\d webhook_events
```

Expected indexes:
- `webhook_events_pkey` (PRIMARY KEY)
- `ux_webhook_events_lead_updated` (UNIQUE)
- `ix_webhook_events_status`
- `ix_webhook_events_received_at`
- `ix_webhook_events_lead_id`

---

### 2. Set Webhook Secret

**Generate a secure secret**:
```bash
# Option 1: Random string
openssl rand -hex 32

# Option 2: UUID
uuidgen
```

**Set in Fly.io**:
```bash
fly secrets set WEBHOOK_SECRET="<your_generated_secret>" -a mbras-c2s
```

**Verify**:
```bash
fly secrets list -a mbras-c2s
```

Should show:
- `WEBHOOK_SECRET` (redacted)
- `DB_URL` (redacted)
- `C2S_TOKEN` (redacted)
- `WORK_API` (redacted)
- etc.

---

### 3. Deploy Code

**Build and deploy**:
```bash
fly deploy -a mbras-c2s
```

**Monitor deployment**:
```bash
fly logs -a mbras-c2s
```

Look for:
```
✓ Configuration loaded successfully
✓ Database connection pool established
✓ C2S Gateway client initialized
✓ Webhook secret configured for C2S webhooks
```

**Verify health**:
```bash
curl https://mbras-c2s.fly.dev/health
```

Expected response:
```json
{
  "status": "healthy",
  "database": "connected"
}
```

---

### 4. Run Integration Tests

**Set test secret** (same as production):
```bash
export WEBHOOK_SECRET="<your_generated_secret>"
```

**Run test suite**:
```bash
./scripts/test_webhook.sh https://mbras-c2s.fly.dev
```

**Expected output**:
```
==================================================
C2S Webhook Endpoint Tests
==================================================
Base URL: https://mbras-c2s.fly.dev
Webhook Secret: <redacted>...

Phase 1: Authentication Tests
----------------------------------------------
Testing: Missing authentication header... ✓ PASS (HTTP 401)
Testing: Invalid authentication token... ✓ PASS (HTTP 401)

Phase 2: Single Event Tests
----------------------------------------------
Testing: Valid single event... ✓ PASS (HTTP 200)
  Response: {"status":"received","received":1,"processed":1,"duplicates":0}
Testing: Duplicate event (idempotency)... ✓ PASS (HTTP 200)
  Response: {"status":"received","received":1,"processed":0,"duplicates":1}
Testing: Same lead, different timestamp... ✓ PASS (HTTP 200)

Phase 3: Batch Event Tests
----------------------------------------------
Testing: Batch of 3 events... ✓ PASS (HTTP 200)

Phase 4: Error Handling Tests
----------------------------------------------
Testing: Missing updated_at field... ✓ PASS (HTTP 400)
Testing: Invalid timestamp format... ✓ PASS (HTTP 400)

==================================================
Test Summary
==================================================
Tests Passed: 8
Tests Failed: 0

All tests passed!
```

---

### 5. Configure C2S Webhook

**In C2S Dashboard**:

1. Navigate to **Settings** → **Webhooks** → **Add Webhook**

2. **Webhook URL**:
   ```
   https://mbras-c2s.fly.dev/api/v1/webhooks/c2s
   ```

3. **HTTP Method**: `POST`

4. **Custom Headers**:
   ```
   X-Webhook-Token: <your_generated_secret>
   ```

5. **Events to Subscribe**:
   - [x] `lead.created`
   - [x] `lead.updated`
   - [x] `lead.status_changed` (optional)

6. **Save** and **Enable**

---

### 6. Test with Real C2S Event

**Option 1: Create test lead in C2S**:
1. Go to C2S dashboard
2. Create new lead manually
3. Monitor webhook receipt

**Option 2: Update existing lead**:
1. Edit any lead in C2S
2. Change a field (e.g., status, notes)
3. Save
4. Check webhook fired

**Monitor webhook events**:
```bash
# Watch logs
fly logs -a mbras-c2s | grep webhook

# Check database
psql $DB_URL
```

```sql
SELECT id, lead_id, status, hook_action, received_at, processed_at
FROM webhook_events
ORDER BY received_at DESC
LIMIT 5;
```

**Expected**:
- New row in `webhook_events`
- Status: `received` → `processing` → `completed`
- Log entry: "Successfully enriched lead_id=..."

---

## Verification Queries

### Check Webhook Activity

```sql
-- Recent webhooks (last hour)
SELECT 
    lead_id,
    hook_action,
    status,
    received_at,
    processed_at,
    EXTRACT(EPOCH FROM (processed_at - received_at)) as processing_time_sec
FROM webhook_events
WHERE received_at > NOW() - INTERVAL '1 hour'
ORDER BY received_at DESC;
```

### Check Processing Status

```sql
-- Status breakdown
SELECT 
    status,
    COUNT(*) as count,
    MIN(received_at) as first_event,
    MAX(received_at) as last_event
FROM webhook_events
GROUP BY status
ORDER BY count DESC;
```

### Check Failures

```sql
-- Failed events (if any)
SELECT 
    lead_id,
    hook_action,
    error_message,
    received_at
FROM webhook_events
WHERE status = 'failed'
ORDER BY received_at DESC;
```

### Check Duplicates

```sql
-- Duplicate webhooks received
SELECT 
    lead_id,
    updated_at,
    COUNT(*) as times_received
FROM webhook_events
GROUP BY lead_id, updated_at
HAVING COUNT(*) > 1
ORDER BY times_received DESC;
```

---

## Rollback Procedure

If issues arise:

### 1. Disable C2S Webhook
- Go to C2S Dashboard → Webhooks
- **Disable** the webhook (don't delete, just disable)
- Re-enable Make.com scenario

### 2. Revert Code (if needed)
```bash
git log --oneline | head -5  # Find commit before webhook
git revert <commit_hash>     # Revert webhook commit
fly deploy -a mbras-c2s      # Redeploy
```

### 3. Keep Data
- Leave `webhook_events` table intact (historical data)
- Migration does not need rollback (idempotent)

---

## Troubleshooting

### Webhook returns 401

**Cause**: Secret mismatch

**Fix**:
```bash
# Check Fly.io secret
fly secrets list -a mbras-c2s

# Update if needed
fly secrets set WEBHOOK_SECRET="<correct_secret>" -a mbras-c2s

# Update C2S header
# Go to C2S Dashboard → Webhooks → Edit → Update X-Webhook-Token
```

### Webhook returns 400

**Cause**: Missing or invalid `updated_at`

**Fix**:
- Check C2S webhook payload structure
- Ensure `attributes.updated_at` field is present
- Verify timestamp format (ISO 8601 / RFC3339)

### Events stuck in "processing"

**Cause**: Enrichment workflow error

**Check logs**:
```bash
fly logs -a mbras-c2s | grep "Failed to enrich"
```

**Manual investigation**:
```sql
SELECT * FROM webhook_events WHERE status = 'processing';
```

**Note**: Current implementation has placeholder enrichment workflow.  
Full workflow (CPF extraction → Work API → C2S message) is TODO.

### Database connection errors

**Check**:
```bash
fly ssh console -a mbras-c2s
echo $DATABASE_URL
psql $DATABASE_URL -c "SELECT 1"
```

**Fix**:
- Verify DB_URL secret is correct
- Check Neon.tech database is online
- Restart app: `fly apps restart mbras-c2s`

---

## Post-Deployment Monitoring

### Day 1: Watch Closely

```bash
# Monitor logs live
fly logs -a mbras-c2s

# Check every 15 minutes
watch -n 900 'psql $DB_URL -c "SELECT status, COUNT(*) FROM webhook_events GROUP BY status"'
```

### Week 1: Daily Check

```sql
-- Daily summary query
SELECT 
    DATE(received_at) as date,
    status,
    COUNT(*) as events
FROM webhook_events
WHERE received_at > NOW() - INTERVAL '7 days'
GROUP BY DATE(received_at), status
ORDER BY date DESC, status;
```

### Ongoing: Alerts

Set up alerts for:
- ⚠️ Error rate > 5%
- ⚠️ Processing time > 30 seconds
- ⚠️ Failed events > 10 per hour
- ⚠️ No webhooks received for > 1 hour (during business hours)

---

## Success Criteria

✅ **Deployment Successful** when:

1. Migration applied without errors
2. Webhook secret set in Fly.io
3. Code deployed and health check passes
4. All 8 integration tests pass
5. C2S webhook configured and enabled
6. Real webhook from C2S received and processed
7. Event status: `received` → `completed`
8. No errors in logs

---

**Last Updated**: 2025-01-20  
**Status**: Ready for deployment
