# Deployment Checklist

Complete pre-deployment and post-deployment checklist for rust-c2s-api.

---

## Pre-Deployment

### 1. Code Quality ✅

- [ ] All tests pass: `cargo test`
- [ ] Code formatted: `cargo fmt --check`
- [ ] No clippy warnings: `cargo clippy -- -D warnings`
- [ ] Build succeeds: `cargo build --release`
- [ ] No compiler warnings

### 2. Environment Configuration ✅

- [ ] `.env` file created from `.env.example`
- [ ] All required env vars set:
  - [ ] `C2S_TOKEN`
  - [ ] `C2S_BASE_URL`
  - [ ] `WORK_API`
  - [ ] `DIRETRIX_BASE_URL`
  - [ ] `DIRETRIX_USER`
  - [ ] `DIRETRIX_PASS`
  - [ ] `DB_URL`
  - [ ] `PORT`
- [ ] Credentials are production values (not test/staging)
- [ ] Database URL points to production Neon instance

### 3. Database ✅

- [ ] Production database accessible
- [ ] Schema applied (check `core.entities`, `core.entity_emails`, etc.)
- [ ] Connection pool size appropriate (default: 10)
- [ ] SSL mode enabled in connection string (`?sslmode=require`)

### 4. Local Testing ✅

- [ ] Integration tests pass: `./test-local.sh`
- [ ] Docker tests pass: `./test-docker.sh`
- [ ] Smoke test passes: `k6 run tests/smoke-test.js`
- [ ] Can trigger lead processing with real lead ID
- [ ] Enriched data appears in C2S
- [ ] Data stored in database

### 5. External APIs ✅

- [ ] C2S API accessible: `curl -H "Authorization: Bearer $C2S_TOKEN" https://api.contact2sale.com/integration/leads/test_id`
- [ ] Diretrix API accessible: `curl http://api.diretrixconsultoria.com.br`
- [ ] Work API accessible: Test with known CPF
- [ ] API credentials valid
- [ ] Rate limits understood

### 6. Fly.io Setup ✅

- [ ] Fly.io account created
- [ ] `fly` CLI installed: `fly version`
- [ ] Logged in: `fly auth login`
- [ ] App created: `fly apps list` shows `rust-c2s-api`
- [ ] `fly.toml` configured correctly
- [ ] Secrets set: `fly secrets list`

---

## Deployment Steps

### 1. Set Secrets on Fly.io

```bash
# Set all secrets (one-time)
fly secrets set \
  C2S_TOKEN="your_token" \
  C2S_BASE_URL="https://api.contact2sale.com" \
  WORK_API="your_work_key" \
  DIRETRIX_BASE_URL="http://api.diretrixconsultoria.com.br" \
  DIRETRIX_USER="your_user" \
  DIRETRIX_PASS="your_pass" \
  DB_URL="postgresql://..." \
  --app rust-c2s-api

# Verify secrets
fly secrets list --app rust-c2s-api
```

### 2. Deploy Application

```bash
# Deploy
fly deploy --app rust-c2s-api

# Watch deployment
fly logs -f --app rust-c2s-api
```

**Expected output:**
```
==> Building image
==> Pushing image to fly
==> Creating release
==> Monitoring deployment
v1 deployed successfully
```

### 3. Verify Deployment

```bash
# Check status
fly status --app rust-c2s-api

# Expected:
# Status   = running
# Health   = passing
# Memory   = ~150 MB / 1024 MB
```

---

## Post-Deployment Validation

### 1. Health Check ✅

```bash
curl https://your-app.fly.dev/health

# Expected: {"status":"ok","service":"rust-c2s-api"}
```

- [ ] Returns 200 OK
- [ ] Response is valid JSON

### 2. Smoke Test ✅

```bash
k6 run -e BASE_URL=https://your-app.fly.dev tests/smoke-test.js
```

- [ ] All requests succeed
- [ ] p95 latency <3s
- [ ] Error rate <5%

### 3. End-to-End Test ✅

```bash
# Get a real lead ID from C2S
LEAD_ID="358f62821dc6cfa7cfbda19e670d6392"

# Trigger processing
curl "https://your-app.fly.dev/api/v1/leads/process?id=$LEAD_ID"

# Expected: {"success":true,"message":"Successfully processed..."}
```

- [ ] Request completes in <10s
- [ ] Returns success=true
- [ ] CPFs found and processed
- [ ] Enriched message in C2S lead timeline
- [ ] Data in database (check Neon console)

### 4. Monitor Logs ✅

```bash
fly logs -f --app rust-c2s-api
```

**Look for:**
- [ ] No error messages
- [ ] Successful lead processing logs
- [ ] Database connections stable
- [ ] External API calls succeeding

**Expected log flow:**
```
INFO  === Trigger Lead Processing: 358f62... ===
INFO  Step 1: Fetching lead from C2S
INFO  ✓ Successfully fetched lead from C2S
INFO  Step 2: Using Diretrix to find CPF
INFO  ✓ Phone and email belong to the same person (CPF: 123...)
INFO  Step 3: Enriching 1 CPF(s) with Work API
INFO  ✓ Enriched CPF: 123...
INFO  Step 4: Formatting enriched data for C2S
INFO  Step 5: Storing 1 person(s) in database
INFO  ✓ Stored CPF 123... → entity_id: ...
INFO  Step 6: Sending enriched data to C2S
INFO  ✓ Successfully sent enriched data to C2S
```

### 5. Database Verification ✅

```sql
-- Connect to Neon database
psql "postgresql://neondb_owner:...@ep-...-pooler.sa-east-1.aws.neon.tech/neondb?sslmode=require"

-- Check recent entities
SELECT entity_id, national_id, name, is_enriched, enriched_at
FROM core.entities
WHERE enriched_at > NOW() - INTERVAL '1 hour'
ORDER BY enriched_at DESC
LIMIT 5;

-- Check entity emails
SELECT ee.email, ee.is_verified, ee.metadata
FROM core.entity_emails ee
JOIN core.entities e ON ee.entity_id = e.entity_id
WHERE e.enriched_at > NOW() - INTERVAL '1 hour';

-- Check entity phones
SELECT ep.phone, ep.is_whatsapp, ep.carrier
FROM core.entity_phones ep
JOIN core.entities e ON ep.entity_id = e.entity_id
WHERE e.enriched_at > NOW() - INTERVAL '1 hour';
```

- [ ] New entities appear
- [ ] `is_enriched = true`
- [ ] `enriched_at` is recent
- [ ] Emails and phones populated

### 6. Resource Monitoring ✅

```bash
# Check resource usage
fly status --app rust-c2s-api
```

- [ ] Memory usage <500 MB
- [ ] CPU usage <50%
- [ ] No OOM kills in logs
- [ ] Response times acceptable

---

## Make.com Integration Update

### 1. Test Endpoint Manually

```bash
# Use Make's lead ID variable format
LEAD_ID="your_test_lead_id"
curl "https://your-app.fly.dev/api/v1/leads/process?id=$LEAD_ID"
```

- [ ] Returns 200 OK
- [ ] JSON contains success=true
- [ ] Lead processed correctly

### 2. Update Make Scenario

**Old configuration:**
```
URL: https://us-central1-xxx.cloudfunctions.net/processLead?id={{lead_id}}
```

**New configuration:**
```
URL: https://your-app.fly.dev/api/v1/leads/process?id={{lead_id}}
Method: GET
Headers: (none needed)
Parse Response: Yes
```

Steps:
- [ ] Open Make scenario
- [ ] Locate HTTP module
- [ ] Update URL field
- [ ] Save scenario
- [ ] Run test execution
- [ ] Verify in Make execution log
- [ ] Check C2S for enriched lead

### 3. Monitor First Real Leads

- [ ] Watch Fly logs: `fly logs -f`
- [ ] Monitor Make execution history
- [ ] Check C2S lead timelines
- [ ] Verify database entries

---

## Performance Baseline

### 1. Run Load Test ✅

```bash
k6 run -e BASE_URL=https://your-app.fly.dev tests/load-test.js
```

**Record metrics:**
- [ ] Total requests: _______
- [ ] p95 latency: _______ ms
- [ ] Error rate: _______ %
- [ ] Peak memory: _______ MB

### 2. Optimize if Needed

**If memory usage <50%:**
```bash
# Reduce to 512 MB
fly scale memory 512 --app rust-c2s-api
# Wait 24h and monitor
```

**If CPU saturated:**
```bash
# Upgrade to dedicated CPU
# Edit fly.toml: cpu_kind = "performance"
fly deploy
```

**If high latency:**
- Check external API response times
- Review database query performance
- Consider adding caching

---

## Rollback Plan

### If Deployment Fails

```bash
# View previous releases
fly releases --app rust-c2s-api

# Rollback to previous version
fly releases rollback v# --app rust-c2s-api

# Verify
fly status --app rust-c2s-api
```

### If Make Integration Fails

1. Revert Make scenario to old Cloud Function URL
2. Investigate Rust service logs
3. Fix issues
4. Redeploy
5. Try Make integration again

---

## Post-Deployment Monitoring (First 24h)

### Hour 1
- [ ] Check logs every 10 minutes
- [ ] Verify first 5-10 leads process successfully
- [ ] Monitor memory usage trend
- [ ] No errors in logs

### Hour 6
- [ ] Review aggregated metrics
- [ ] Check database growth
- [ ] Verify no OOM kills
- [ ] Latency within expected range

### Hour 24
- [ ] Run smoke test again
- [ ] Review error rate
- [ ] Check resource usage trend
- [ ] Optimize if needed

---

## Success Criteria

✅ **Deployment Successful If:**
- Application status = running
- Health check returns 200
- Smoke tests pass
- End-to-end test successful
- Make.com integration working
- Data appearing in database
- No critical errors in logs
- Memory usage <700 MB
- Response times <5s (p95)

🚨 **Rollback If:**
- Application won't start
- Health check fails
- >10% error rate
- OOM kills occurring
- External APIs unreachable
- Database connection fails
- Make.com integration broken

---

## Ongoing Maintenance

### Weekly
- [ ] Review error logs
- [ ] Check resource usage trends
- [ ] Verify external API quotas

### Monthly
- [ ] Run full load test
- [ ] Review capacity vs. traffic
- [ ] Update dependencies: `cargo update`
- [ ] Security audit: `cargo audit`
- [ ] Review database query performance

### Quarterly
- [ ] Review architecture
- [ ] Optimize hot paths
- [ ] Update Rust version
- [ ] Review cost optimization

---

## Contacts & Escalation

**Fly.io Issues:**
- Dashboard: https://fly.io/dashboard
- Support: https://community.fly.io
- Status: https://status.fly.io

**External APIs:**
- C2S Support: [contact info]
- Diretrix Support: [contact info]
- Work API Support: [contact info]

**Database:**
- Neon Console: https://console.neon.tech
- Neon Support: [contact info]

---

Generated: 2025-01-13

**Last Deployment:** _________________
**Deployed By:** _________________
**Version:** _________________
**Notes:** _________________
