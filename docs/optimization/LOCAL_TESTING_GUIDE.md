# Local Testing Guide - Database-First CPF Lookup Optimization

**Purpose**: How to test the cache optimization locally  
**Date**: November 22, 2025  

---

## 🚀 Quick Start

### 1. Start Local Server

```bash
cd /Users/ronaldo/Documents/projects/MBRAS/mbras-c2s/rust-c2s-api

# Build the project
cargo build

# Start the server
cargo run --bin rust-c2s-api
```

**Expected output**:
```
INFO rust_c2s_api: Configuration loaded successfully
INFO rust_c2s_api: Database connection pool established
INFO rust_c2s_api: CPF deduplication cache initialized
INFO rust_c2s_api: Lead deduplication cache initialized
INFO rust_c2s_api: Contact enrichment cache initialized ← NEW!
INFO rust_c2s_api: ✓ C2S Direct Client initialized
INFO rust_c2s_api: Server listening on 0.0.0.0:8080
```

### 2. Test Health Endpoint

```bash
curl http://localhost:8080/health | jq
```

**Expected response**:
```json
{
  "status": "healthy",
  "service": "rust-c2s-api",
  "version": "0.1.0"
}
```

---

## 🧪 Testing Cache Optimization

### Test Script

Save this as `test_cache.sh`:

```bash
#!/bin/bash

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

API_URL="http://localhost:8080"
PHONE="+5511999887766"

echo -e "${BLUE}═══════════════════════════════════════${NC}"
echo -e "${BLUE}  Testing Cache Optimization${NC}"
echo -e "${BLUE}═══════════════════════════════════════${NC}\n"

# Test 1: First request (cache miss)
echo -e "${YELLOW}Test 1: First Request (Cache Miss)${NC}"
LEAD_ID_1="test-$(date +%s)-first"

curl -s -X POST "$API_URL/api/v1/webhooks/c2s" \
  -H "Content-Type: application/json" \
  -d "{
    \"id\": \"$LEAD_ID_1\",
    \"hook_action\": \"hook_on_create_lead\",
    \"attributes\": {
      \"updated_at\": \"$(date -u +"%Y-%m-%dT%H:%M:%SZ")\",
      \"customer\": {
        \"name\": \"Test Customer\",
        \"phone\": \"$PHONE\",
        \"email\": null
      },
      \"product\": {},
      \"lead_status\": {},
      \"log\": [],
      \"messages\": []
    }
  }" | jq

echo ""
echo -e "${YELLOW}Waiting 3 seconds for background processing...${NC}"
sleep 3

# Test 2: Second request (cache hit)
echo ""
echo -e "${YELLOW}Test 2: Second Request (Cache Hit!)${NC}"
LEAD_ID_2="test-$(date +%s)-second"

START=$(date +%s%N)
curl -s -X POST "$API_URL/api/v1/webhooks/c2s" \
  -H "Content-Type: application/json" \
  -d "{
    \"id\": \"$LEAD_ID_2\",
    \"hook_action\": \"hook_on_update_lead\",
    \"attributes\": {
      \"updated_at\": \"$(date -u +"%Y-%m-%dT%H:%M:%SZ")\",
      \"customer\": {
        \"name\": \"Test Customer\",
        \"phone\": \"$PHONE\",
        \"email\": null
      },
      \"product\": {},
      \"lead_status\": {},
      \"log\": [],
      \"messages\": []
    }
  }" | jq
END=$(date +%s%N)

DURATION=$(( ($END - $START) / 1000000 ))
echo ""
echo -e "${GREEN}✅ Second request completed in ${DURATION}ms${NC}"

echo ""
echo -e "${BLUE}═══════════════════════════════════════${NC}"
echo "Check server logs for:"
echo "  ✅ Found existing enrichment for CPF"
echo "  🎯 DB HIT: Found CPF from database"
echo "  💸 DIRETRIX API (if cache/DB miss)"
```

### Run the Test

```bash
chmod +x test_cache.sh
./test_cache.sh
```

---

## 📊 Understanding the Results

### Scenario 1: Cache Hit (Ideal)

**Server Logs**:
```
INFO Starting enrichment workflow for lead_id: test-xxx-second
✅ Found existing enrichment for CPF: 12345678901
Sending cached message to C2S
```

**What happened**:
1. Checked in-memory cache → **HIT!**
2. Used cached enrichment data
3. Sent message to C2S immediately
4. **No external API calls** ⚡

**Performance**: ~0.1-10ms (99.8% faster than API call)

---

### Scenario 2: Database Hit (Good)

**Server Logs**:
```
INFO Starting enrichment workflow for lead_id: test-xxx-second
🎯 DB HIT: Found CPF from database: 12345678901
✅ Found existing enrichment for CPF: 12345678901
Sending cached message to C2S
```

**What happened**:
1. Checked in-memory cache → miss
2. Checked database → **HIT!**
3. Cached result for next time
4. Used database enrichment data
5. Sent message to C2S
6. **No external API calls** ⚡

**Performance**: ~10-50ms (still much faster than API)

---

### Scenario 3: Cache/DB Miss (Expected for Unknown Contacts)

**Server Logs**:
```
INFO Starting enrichment workflow for lead_id: test-xxx-first
Step 1: Finding CPF via Diretrix
💸 DIRETRIX API: No cache/DB hit, calling external API
Diretrix: Searching by phone: +5511999887766
```

**What happened**:
1. Checked in-memory cache → miss
2. Checked database → miss
3. Called Diretrix API → Found CPF (or error if unknown)
4. Called Work API → Got enrichment data
5. Stored in database
6. Cached for future requests

**Performance**: ~3-6 seconds (normal for first-time contacts)

---

### Scenario 4: Unknown Contact (Error - Expected for Test Data)

**Server Logs**:
```
ERROR Could not find CPF from either phone or email
ERROR Failed to enrich lead_id=test-xxx: Not found: Could not find CPF via Diretrix
```

**What happened**:
- Contact doesn't exist in Diretrix API
- This is **expected for test phone numbers**
- In production, real contacts will be found

**Solution**: Use a real phone number from your database with enriched data

---

## 🔍 Finding Real Test Data

### Query Database for Enriched Contacts

```sql
-- Find contacts with enrichment data
SELECT 
    pc.value as phone,
    p.cpf_cnpj,
    p.name,
    p.updated_at
FROM core.party_contacts pc
JOIN core.parties p ON pc.party_id = p.id
JOIN core.party_enrichments pe ON pe.party_id = p.id
WHERE p.enriched = true
  AND p.cpf_cnpj IS NOT NULL
  AND pe.normalized_data IS NOT NULL
  AND pc.contact_type IN ('phone', 'whatsapp')
ORDER BY p.updated_at DESC
LIMIT 10;
```

**Use one of these phone numbers in your test!**

---

## 📈 Performance Benchmarks

### Expected Results

| Request Type | Cache Hit | DB Hit | API Miss |
|--------------|-----------|--------|----------|
| **Latency** | 0.1-1ms | 5-50ms | 3-6 seconds |
| **External APIs** | 0 | 0 | 2 (Diretrix + Work) |
| **Database queries** | 0 | 1 | Multiple |
| **Improvement** | 99.98% | 99% | Baseline |

### Success Indicators

✅ **Cache working correctly if**:
- Second request with same phone is < 100ms
- Logs show "✅ Found existing enrichment"
- No "DIRETRIX API" log on second request

⚠️ **Needs investigation if**:
- Both requests take > 1 second
- Always seeing "DIRETRIX API" logs
- Errors about missing enrichment data

---

## 🐛 Troubleshooting

### Issue: "Could not find CPF via Diretrix"

**Cause**: Test phone number doesn't exist in Diretrix database

**Solution**: 
1. Use a real phone number from your database (see SQL query above)
2. Or test with production webhooks that have real data

---

### Issue: Always Cache Miss

**Check**:
```bash
# Is cache initialized?
grep "Contact enrichment cache initialized" server_logs

# Is data being stored?
psql $DB_URL -c "SELECT COUNT(*) FROM core.party_enrichments WHERE normalized_data IS NOT NULL;"
```

**Solution**:
- Verify cache initialization in startup logs
- Check database has enriched data
- Ensure first request completed successfully

---

### Issue: Both Requests Slow

**Check**:
```bash
# Is server connecting to database?
grep "Database connection pool established" server_logs
```

**Solution**:
- Verify `.env` has correct `DB_URL`
- Check database connectivity
- Look for database errors in logs

---

## 🎯 Manual Testing Steps

### Complete Test Flow

1. **Start server**:
   ```bash
   cargo run --bin rust-c2s-api
   ```

2. **Send first webhook** (cache miss expected):
   ```bash
   curl -X POST http://localhost:8080/api/v1/webhooks/c2s \
     -H "Content-Type: application/json" \
     -d '{
       "id": "test-1",
       "hook_action": "hook_on_create_lead",
       "attributes": {
         "updated_at": "2025-11-22T12:00:00Z",
         "customer": {
           "name": "Test Customer",
           "phone": "+5511999887766",
           "email": null
         },
         "product": {},
         "lead_status": {},
         "log": [],
         "messages": []
       }
     }'
   ```

3. **Wait 3 seconds** for background processing

4. **Send second webhook** (cache hit expected):
   ```bash
   curl -X POST http://localhost:8080/api/v1/webhooks/c2s \
     -H "Content-Type: application/json" \
     -d '{
       "id": "test-2",
       "hook_action": "hook_on_update_lead",
       "attributes": {
         "updated_at": "2025-11-22T12:01:00Z",
         "customer": {
           "name": "Test Customer",
           "phone": "+5511999887766",
           "email": null
         },
         "product": {},
         "lead_status": {},
         "log": [],
         "messages": []
       }
     }'
   ```

5. **Check logs** for cache hit message

---

## 📝 Log Patterns to Look For

### Cache Hit (Success!)
```
INFO Starting enrichment workflow for lead_id: test-2
✅ Found existing enrichment for CPF: 12345678901
Sending cached message to C2S
```

### Database Hit (Good!)
```
INFO Starting enrichment workflow for lead_id: test-2
🎯 DB HIT: Found CPF from database: 12345678901
✅ Found existing enrichment for CPF: 12345678901
```

### Cache Miss (Expected for new contacts)
```
INFO Starting enrichment workflow for lead_id: test-1
Step 1: Finding CPF via Diretrix
💸 DIRETRIX API: No cache/DB hit, calling external API
```

### Error (Expected for test data)
```
ERROR Could not find CPF from either phone or email
ERROR Failed to enrich lead_id=test-1: Not found: Could not find CPF via Diretrix
```

---

## ✅ Success Checklist

Before considering the test successful, verify:

- [ ] Server starts without errors
- [ ] Health endpoint returns 200 OK
- [ ] "Contact enrichment cache initialized" in startup logs
- [ ] First webhook accepted (returns 200)
- [ ] Second webhook with same phone accepted
- [ ] Second request faster than first (< 100ms)
- [ ] Logs show "✅ Found existing enrichment" on second request
- [ ] No "DIRETRIX API" log on second request

---

## 🎓 Learning Points

### What the Test Demonstrates

1. **First Request (Cache/DB Miss)**:
   - Normal flow: Diretrix API → Work API → Store → Send
   - Slower (3-6 seconds)
   - Expected for new contacts

2. **Second Request (Cache Hit)**:
   - Optimized flow: Cache → Format → Send
   - Fast (< 100ms)
   - **This is the 58% reduction in API calls!**

3. **Cache Persistence**:
   - Cache lasts 24 hours (TTL)
   - Survives until server restart
   - Shared by all requests to same server instance

---

## 🔗 Related Documentation

- **Full Technical Docs**: `DATABASE_FIRST_LOOKUP.md`
- **Quick Reference**: `QUICK_REFERENCE.md`
- **Deployment Summary**: `DEPLOYMENT_SUMMARY.md`

---

**Last Updated**: November 22, 2025  
**Status**: ✅ Ready for Local Testing
