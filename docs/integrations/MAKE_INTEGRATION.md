# Make.com Integration - Lead Processing Trigger

## Overview
This document describes the interim trigger endpoint for Make.com to process C2S leads without requiring a Cloud Function. The Rust service now accepts lead IDs directly from Make, fetches the full lead data from C2S, and processes it through the complete enrichment pipeline.

---

## Endpoint Details

### GET /api/v1/leads/process

**Purpose:** Trigger endpoint for Make.com to initiate lead processing

**Method:** `GET`

**Query Parameters:**
- `id` (required): The C2S lead ID to process

**Example Request:**
```bash
curl "https://your-rust-service.fly.dev/api/v1/leads/process?id=358f62821dc6cfa7cfbda19e670d6392"
```

**Response Format:**
```json
{
  "success": true,
  "message": "Successfully processed and enriched lead. Stored 1 entities in database.",
  "lead_id": "358f62821dc6cfa7cfbda19e670d6392",
  "cpfs_processed": ["12345678900"],
  "entities_stored": 1
}
```

**Error Response:**
```json
{
  "success": false,
  "message": "Could not find CPF from phone or email",
  "lead_id": "358f62821dc6cfa7cfbda19e670d6392"
}
```

---

## Processing Flow

The endpoint executes the following steps:

### 1. **Fetch Lead from C2S**
- Uses `C2S_TOKEN` and `C2S_BASE_URL` from environment
- Calls `GET /integration/leads/{lead_id}` on C2S API
- Extracts customer name, phone, and email

### 2. **Find CPF via Diretrix**
- Parallel lookup using phone AND email
- Handles three scenarios:
  - **Same person**: Both phone and email resolve to same CPF → adds indicator message
  - **Different people**: Phone and email resolve to different CPFs → enriches both
  - **Single source**: Only one available → uses that CPF

### 3. **Enrich with Work API**
- Fetches complete data for each CPF found
- Includes: personal info, financials, addresses, contacts
- Applies 1.9x multiplier to income values

### 4. **Format Message**
- Creates enriched message with:
  - Phone/email match indicator (if applicable)
  - Personal details
  - Financial information
  - Credit score and risk level
  - Contact information

### 5. **Store in Database**
- Saves enriched data to production database
- Tables used:
  - `core.entities` - Person record
  - `core.entity_profiles` - Personal details
  - `core.entity_financials` - Financial data
  - `core.entity_emails` - Email contacts
  - `core.entity_phones` - Phone contacts
  - `core.entity_addresses` - Address information

### 6. **Send to C2S**
- Posts enriched message back to C2S
- Uses `POST /integration/leads/{lead_id}/create_message`
- Message appears in C2S lead timeline

---

## Make.com Setup

### Current Configuration (Cloud Function)
```
HTTP Module → Cloud Function
URL: https://us-central1-xxx.cloudfunctions.net/processLead?id={{lead_id}}
```

### New Configuration (Rust Service)
```
HTTP Module → Rust Service
URL: https://your-rust-service.fly.dev/api/v1/leads/process?id={{lead_id}}
Method: GET
Headers: None required (service uses env vars)
```

### Migration Steps

1. **Deploy Rust Service:**
   ```bash
   # Ensure .env has all required variables
   cp .env.example .env
   # Edit .env with production credentials
   
   # Deploy to Fly.io
   fly deploy
   ```

2. **Test the Endpoint:**
   ```bash
   # Get a test lead ID from C2S
   curl "https://your-app.fly.dev/api/v1/leads/process?id=TEST_LEAD_ID"
   
   # Verify:
   # - Lead fetched from C2S
   # - CPF found via Diretrix
   # - Work API enrichment successful
   # - Data stored in database
   # - Message sent back to C2S
   ```

3. **Update Make.com Scenario:**
   - Open the Make scenario
   - Locate the HTTP module that calls Cloud Function
   - Update URL to: `https://your-app.fly.dev/api/v1/leads/process?id={{lead_id}}`
   - Keep query parameter as `?id={{lead_id}}`
   - Save and test

4. **Verify Make Integration:**
   - Trigger a test run in Make
   - Check Make execution log for HTTP 200 response
   - Verify lead appears enriched in C2S
   - Check database for stored entity

5. **Monitor and Retire Cloud Function:**
   - Monitor Rust service logs: `fly logs`
   - After successful testing, disable Cloud Function
   - Update documentation to reflect new flow

---

## Environment Variables Required

```bash
# C2S API
C2S_TOKEN=your_c2s_token
C2S_BASE_URL=https://api.contact2sale.com

# Work API
WORK_API=your_work_api_key

# Diretrix API
DIRETRIX_BASE_URL=http://api.diretrixconsultoria.com.br
DIRETRIX_USER=your_username
DIRETRIX_PASS=your_password

# Database
DB_URL=postgresql://user:pass@host:port/database?sslmode=require

# Server
PORT=8081
```

---

## Success Criteria

### HTTP Status Codes
- `200 OK` - Lead processed successfully (check JSON `success` field)
- `400 Bad Request` - Missing or invalid lead ID
- `502 Bad Gateway` - C2S, Diretrix, or Work API unavailable
- `500 Internal Server Error` - Database or processing error

### Successful Processing Indicators
- ✅ `"success": true` in response
- ✅ `cpfs_processed` array contains at least one CPF
- ✅ `entities_stored` > 0
- ✅ Enriched message visible in C2S lead timeline
- ✅ Entity record exists in database

### Error Scenarios
- ❌ Lead ID not found in C2S → 502 error
- ❌ No CPF found from phone/email → success=false response
- ❌ Work API enrichment failed → success=false response
- ❌ C2S message send failed → success=false but data still stored

---

## Logging

View real-time logs:
```bash
fly logs
```

Expected log flow for successful processing:
```
INFO  === Trigger Lead Processing: 358f62821dc6cfa7cfbda19e670d6392 ===
INFO  Step 1: Fetching lead from C2S
INFO  ✓ Successfully fetched lead from C2S
INFO  Lead details - Customer: João Silva, Phone: 5511999998888, Email: joao@example.com
INFO  Step 2: Using Diretrix to find CPF
INFO  ✓ Phone and email belong to the same person (CPF: 12345678900)
INFO  Step 3: Enriching 1 CPF(s) with Work API
INFO  ✓ Enriched CPF: 12345678900
INFO  Step 4: Formatting enriched data for C2S
INFO  Formatted message length: 847 chars
INFO  Step 5: Storing 1 person(s) in database
INFO  ✓ Stored CPF 12345678900 → entity_id: f47ac10b-58cc-4372-a567-0e02b2c3d479
INFO  Step 6: Sending enriched data to C2S
INFO  ✓ Successfully sent enriched data to C2S for lead: 358f62821dc6cfa7cfbda19e670d6392
```

---

## Advantages Over Cloud Function

1. **No Data Duplication**
   - Cloud Function required Make to send full lead payload
   - New endpoint fetches directly from C2S (single source of truth)

2. **Simplified Make Scenario**
   - Only need to pass lead ID
   - No JSON payload construction in Make
   - Fewer failure points

3. **Better Error Handling**
   - Detailed JSON responses with error context
   - Proper HTTP status codes
   - Comprehensive logging

4. **Database Persistence**
   - All enriched data stored automatically
   - No separate storage logic needed
   - Ready for analytics and reporting

5. **Consistent with Future Webhook**
   - Same processing logic as eventual C2S webhook
   - Easy migration when webhook becomes available

---

## Future Migration to C2S Webhook

When C2S webhook is ready, the flow will change to:

**Current (Make trigger):**
```
C2S → Make → Rust Service (trigger endpoint)
```

**Future (Direct webhook):**
```
C2S → Rust Service (webhook endpoint)
```

No changes needed to enrichment logic - only the entry point changes from GET `/api/v1/leads/process?id=...` to POST `/webhook/c2s` with full payload.

---

## Troubleshooting

### "Missing 'id' parameter"
- Ensure Make passes `?id={{lead_id}}` in URL
- Check Make variable mapping

### "Failed to fetch lead from C2S"
- Verify `C2S_TOKEN` is correct
- Check `C2S_BASE_URL` matches production
- Confirm lead ID exists in C2S

### "Could not find CPF from phone or email"
- Lead has invalid/missing phone and email
- Diretrix API may be down
- Phone/email not in Diretrix database

### "Failed to enrich any CPFs"
- Work API may be down
- API key invalid or expired
- CPF not found in Work database

### Database connection errors
- Check `DB_URL` is correct
- Verify network connectivity to Neon
- Confirm connection pool isn't exhausted

---

## Example Make.com HTTP Module Configuration

```json
{
  "url": "https://your-app.fly.dev/api/v1/leads/process",
  "method": "GET",
  "qs": {
    "id": "{{lead.id}}"
  },
  "parseResponse": true,
  "timeout": 30000
}
```

**Expected Response Handling in Make:**
- Map `success` → Router module (true/false path)
- Map `message` → Email notification on failure
- Map `entities_stored` → Analytics/metrics

---

Generated: 2025-01-13
