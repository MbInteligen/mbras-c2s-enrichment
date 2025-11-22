# Google Ads Integration - C2S API Limitation

**Date**: 2025-11-21  
**Status**: ⚠️ Blocked - C2S API does not support lead creation  
**Severity**: Critical

---

## Problem Summary

The Google Ads Lead Form integration was designed to:
1. Receive lead data from Google Ads webhook
2. Enrich lead with Diretrix + Work API
3. **Create new lead in C2S** with enriched data

However, testing revealed that **C2S API does not support creating new leads** via the integration API.

---

## Evidence

### Test Results

All attempts to create leads via `POST /integration/leads` fail with:

```bash
$ curl -X POST "https://api.contact2sale.com/integration/leads" \
  -H "Authorization: Bearer $C2S_TOKEN" \
  -d '{"customer": "Test", "seller_id": "..."}'

{"message":"param_data_not_found"}
```

**Test Script**: `scripts/test_c2s_create_lead.sh`  
**Result**: All 6 test cases failed with 400 Bad Request

### Supported C2S Integration Endpoints

Based on `src/services.rs` analysis:

✅ **Supported**:
- `GET /integration/leads/{id}` - Fetch lead details
- `POST /integration/leads/{id}/create_message` - Send message to existing lead

❌ **Not Supported**:
- `POST /integration/leads` - Create new lead (returns `param_data_not_found`)

---

## Impact on Google Ads Integration

### Current Implementation Status

| Component | Status | Notes |
|-----------|--------|-------|
| Database migration | ✅ Applied | `google_ads_leads` table exists |
| Webhook handler | ✅ Implemented | `src/google_ads_handler.rs` |
| Authentication | ✅ Working | `google_key` validation works |
| Enrichment logic | ✅ Ready | Diretrix + Work API integration |
| C2S lead creation | ❌ **Blocked** | API endpoint doesn't exist |

### What Doesn't Work

```rust
// This call fails in google_ads_handler.rs:
let c2s_lead_id = gateway_client
    .create_lead(
        &customer_name,
        phone_validated.as_deref(),
        email_validated.as_deref(),
        &description_final,
        Some("Google Ads"),
        app_state.config.c2s_default_seller_id.as_deref(),
    )
    .await?;  // ← Returns "External service error" (400 from C2S)
```

**Error Chain**:
1. Rust handler calls `gateway_client.create_lead()`
2. Gateway forwards to C2S: `POST /integration/leads`
3. C2S returns: `{"message":"param_data_not_found"}`
4. Error bubbles up as "External service error"

---

## Proposed Solutions

### Option 1: Manual Lead Creation Flow ⭐ Recommended

**How it works**:
1. Google Ads webhook receives lead → Store in `google_ads_leads` table with status `pending`
2. User manually creates lead in C2S dashboard
3. C2S webhook fires `on_create_lead` → Our webhook handler enriches it
4. Match Google Ads lead to C2S lead (by phone/email)
5. Update `google_ads_leads.c2s_lead_id` to link them

**Pros**:
- ✅ Uses existing working infrastructure (C2S webhooks)
- ✅ No dependency on non-existent C2S API
- ✅ Full enrichment capability
- ✅ Proper deduplication

**Cons**:
- ❌ Manual step required (not fully automated)
- ❌ Delay between lead capture and C2S creation
- ❌ Risk of leads being forgotten

**Implementation**:
```sql
-- New fields for google_ads_leads table:
ALTER TABLE google_ads_leads 
ADD COLUMN status VARCHAR(20) DEFAULT 'pending',
ADD COLUMN matched_at TIMESTAMPTZ,
ADD COLUMN match_method VARCHAR(50);

-- Status values: 'pending', 'matched', 'imported', 'failed'
```

### Option 2: C2S Dashboard API (if available)

**Research needed**: Check if C2S has a different API for lead creation (not `/integration/*`).

Possible endpoints to investigate:
- `POST /api/leads` (non-integration endpoint)
- `POST /v2/leads` (newer API version)
- Third-party integration partners (Zapier, Make.com)

**Action**: Contact C2S support to ask about lead creation capabilities.

### Option 3: Alternative CRM Integration

If C2S doesn't support programmatic lead creation, consider:
- Using a different CRM that supports it
- Building a custom lead management system
- Using Make.com/Zapier as intermediary

### Option 4: CSV Export + Bulk Import

**How it works**:
1. Store Google Ads leads in PostgreSQL
2. Export to CSV daily/hourly
3. Use C2S bulk import feature (if available)

**Pros**:
- ✅ Simple implementation
- ✅ Works with any CRM

**Cons**:
- ❌ Not real-time
- ❌ Manual process
- ❌ No immediate enrichment

---

## Immediate Actions

### 1. Document Current State ✅

This document serves as the documentation.

### 2. Update DEPLOYMENT_READY.md

Mark Google Ads integration as "Blocked - Awaiting C2S API support"

### 3. Contact C2S Support

**Question to ask**:
> "Does the C2S Integration API support creating new leads programmatically? We need to integrate Google Ads Lead Forms and create leads automatically. The POST /integration/leads endpoint returns 'param_data_not_found'. Is there an alternative endpoint or method?"

### 4. Implement Option 1 (Manual Flow) as Temporary Solution

**Changes needed**:
1. Add `status` field to `google_ads_leads` table
2. Modify webhook handler to store lead without creating in C2S
3. Add matching logic in `on_create_lead` webhook
4. Create admin dashboard to view pending Google Ads leads

---

## Testing Notes

### What Works ✅

- Webhook authentication (`google_key` validation)
- Payload parsing (Google Ads format)
- Enrichment logic (Diretrix + Work API)
- Database storage (`google_ads_leads` table)

### What's Blocked ❌

- Creating leads in C2S
- End-to-end Google Ads → C2S flow
- Production deployment

### Test Commands

```bash
# Test webhook endpoint (payload accepted, but C2S creation fails)
curl -X POST "https://mbras-c2s.fly.dev/api/v1/webhooks/google-ads?google_key=..." \
  -H "Content-Type: application/json" \
  -d '{
    "lead_id": "test-123",
    "api_version": "v15",
    "form_id": 123456,
    "campaign_id": 789,
    "google_key": "...",
    "is_test": true,
    "user_column_data": [
      {"column_id": "FULL_NAME", "column_name": "Nome", "string_value": "Test"},
      {"column_id": "EMAIL", "column_name": "Email", "string_value": "test@example.com"}
    ]
  }'

# Expected: {"error":"External service error"}
# Root cause: C2S API doesn't support POST /integration/leads
```

---

## Related Files

- `src/google_ads_handler.rs` - Webhook handler (implemented but blocked)
- `src/gateway_client.rs` - C2S Gateway client (create_lead method doesn't work)
- `migrations/003_google_ads_leads.sql` - Database schema (applied)
- `scripts/test_c2s_create_lead.sh` - Test script proving API limitation
- `docs/GOOGLE_ADS_INTEGRATION.md` - Original design (based on incorrect assumption)

---

## Conclusion

The Google Ads Lead Form integration is **85% complete** but blocked by a critical limitation: **C2S API does not support creating new leads**.

**Next Steps**:
1. ✅ Document limitation (this file)
2. ⏳ Contact C2S support for clarification
3. ⏳ Implement Option 1 (manual flow) as temporary workaround
4. ⏳ Update architecture docs to reflect reality

**Timeline**:
- With C2S API support: 1-2 days to complete
- Without API support: 3-5 days to implement alternative flow

---

**Author**: AI Assistant  
**Reviewed by**: [Pending]  
**Last Updated**: 2025-11-21
