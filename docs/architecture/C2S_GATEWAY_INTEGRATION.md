# C2S Gateway Integration Plan

**Date**: 2025-11-20  
**Status**: 📋 Planned  
**Purpose**: Integrate Rust C2S API with Python C2S Gateway for centralized C2S operations

---

## Current Architecture

```
┌─────────────┐
│  Make.com   │
│   Webhook   │
└──────┬──────┘
       │
       ↓
┌─────────────────────────┐
│   Rust C2S API          │
│   (mbras-c2s.fly.dev)   │
│                         │
│  ┌──────────────────┐   │
│  │ Work API Client  │───┼──→ Work API (enrichment)
│  ├──────────────────┤   │
│  │ Diretrix Client  │───┼──→ Diretrix API (CPF lookup)
│  ├──────────────────┤   │
│  │ C2S Client       │───┼──→ Contact2Sale API (direct)
│  └──────────────────┘   │
└─────────────────────────┘
```

---

## Proposed Architecture

```
┌─────────────┐
│  Make.com   │
│   Webhook   │
└──────┬──────┘
       │
       ↓
┌─────────────────────────────────────────┐
│   Rust C2S API (mbras-c2s.fly.dev)      │
│                                         │
│  ┌──────────────────┐                   │
│  │ Work API Client  │───┼──→ Work API   │
│  ├──────────────────┤                   │
│  │ Diretrix Client  │───┼──→ Diretrix   │
│  ├──────────────────┤                   │
│  │ Gateway Client   │───┼──┐            │
│  └──────────────────┘   │  │            │
└──────────────────────────┼──┼────────────┘
                          │  │
                          │  ↓
                          │  ┌────────────────────────┐
                          │  │  C2S Gateway (Python)   │
                          │  │  (FastAPI)              │
                          │  │                         │
                          │  │  - 28+ C2S endpoints    │
                          │  │  - Token management     │
                          │  │  - Pydantic validation  │
                          │  │  - Campaign enrichment  │
                          │  └──────────┬──────────────┘
                          │             │
                          │             ↓
                          └───────→ Contact2Sale API
```

---

## Benefits

### 1. **Single Source of Truth**
- All C2S operations go through one gateway
- Consistent error handling and logging
- Centralized token management

### 2. **Better Type Safety**
- Gateway provides Pydantic-validated responses
- Rust client gets strongly-typed structs
- Reduced runtime errors

### 3. **Feature Reuse**
- Campaign enrichment already built in gateway
- Property mapping (campaign_mapping.json)
- Tag management, seller assignment, etc.

### 4. **Easier Maintenance**
- C2S API changes require updating only the gateway
- Rust code becomes simpler (fewer direct API calls)
- Better separation of concerns

### 5. **Enhanced Capabilities**
- Access to all 28+ C2S endpoints
- Distribution queue management
- Webhook subscriptions
- Full CRUD on leads, tags, sellers

---

## Implementation Plan

### Phase 1: Deploy C2S Gateway ✅

**Status**: Gateway already exists and is ready!

**Location**: `/Users/ronaldo/Documents/projects/clients/ibvi/services/ads/platform/c2s-gateway/`

**Endpoints Available**:
- ✅ `GET /leads` - List leads
- ✅ `GET /leads/{id}` - Get specific lead
- ✅ `POST /leads/{id}/messages` - Add message
- ✅ `PATCH /leads/{id}` - Update lead
- ✅ 24+ more endpoints

**Deployment**:
```bash
cd /Users/ronaldo/Documents/projects/clients/ibvi/services/ads/platform/c2s-gateway
fly deploy
```

### Phase 2: Add Gateway Client to Rust API

**File**: `src/services.rs`

**New Struct**:
```rust
pub struct C2sGatewayClient {
    client: reqwest::Client,
    base_url: String,
}

impl C2sGatewayClient {
    pub fn new(base_url: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");
        
        Self { client, base_url }
    }
    
    /// Get lead from C2S via gateway
    pub async fn get_lead(&self, lead_id: &str) -> Result<C2sLead, AppError> {
        let url = format!("{}/leads/{}", self.base_url, lead_id);
        
        let response = self.client
            .get(&url)
            .send()
            .await?
            .json::<C2sLead>()
            .await?;
        
        Ok(response)
    }
    
    /// Send message to lead via gateway
    pub async fn send_message(&self, lead_id: &str, message: &str) -> Result<(), AppError> {
        let url = format!("{}/leads/{}/messages", self.base_url, lead_id);
        
        let body = serde_json::json!({
            "message": message
        });
        
        self.client
            .post(&url)
            .json(&body)
            .send()
            .await?;
        
        Ok(())
    }
    
    /// Update lead via gateway
    pub async fn update_lead(&self, lead_id: &str, data: serde_json::Value) -> Result<(), AppError> {
        let url = format!("{}/leads/{}", self.base_url, lead_id);
        
        self.client
            .patch(&url)
            .json(&data)
            .send()
            .await?;
        
        Ok(())
    }
}
```

### Phase 3: Update Configuration

**File**: `src/config.rs`

**Add**:
```rust
pub struct Config {
    pub database_url: String,
    pub work_api_key: String,
    pub c2s_token: String,
    pub c2s_base_url: String,
    pub c2s_gateway_url: String,  // ← NEW!
    pub diretrix_base_url: String,
    pub diretrix_user: String,
    pub diretrix_pass: String,
    pub port: u16,
}
```

**Environment Variable**:
```env
C2S_GATEWAY_URL=https://c2s-gateway.fly.dev
```

### Phase 4: Update Handlers

**File**: `src/handlers.rs`

**Before** (direct C2S call):
```rust
pub async fn c2s_enrich_lead(
    Path(lead_id): Path<String>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    // Fetch lead from C2S directly
    let lead_data = state.c2s_client.get_lead(&lead_id).await?;
    
    // ... enrichment logic ...
    
    // Send message directly to C2S
    state.c2s_client.send_message(&lead_id, &formatted_message).await?;
}
```

**After** (via gateway):
```rust
pub async fn c2s_enrich_lead(
    Path(lead_id): Path<String>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    // Fetch lead via gateway
    let lead_data = state.gateway_client.get_lead(&lead_id).await?;
    
    // ... enrichment logic ...
    
    // Send message via gateway
    state.gateway_client.send_message(&lead_id, &formatted_message).await?;
}
```

### Phase 5: Update AppState

**File**: `src/main.rs`

**Before**:
```rust
pub struct AppState {
    pub db: DatabaseConnection,
    pub c2s_client: C2sClient,
    pub work_api_client: WorkApiClient,
    pub diretrix_client: DiretrixClient,
    pub processing_leads_cache: Cache<String, i64>,
    pub recent_cpf_cache: Cache<String, i64>,
}
```

**After**:
```rust
pub struct AppState {
    pub db: DatabaseConnection,
    pub c2s_client: C2sClient,          // ← Keep for backward compatibility (optional)
    pub gateway_client: C2sGatewayClient, // ← NEW! Use this instead
    pub work_api_client: WorkApiClient,
    pub diretrix_client: DiretrixClient,
    pub processing_leads_cache: Cache<String, i64>,
    pub recent_cpf_cache: Cache<String, i64>,
}
```

---

## Migration Strategy

### Option A: Gradual Migration (Recommended)

1. ✅ Deploy C2S Gateway to Fly.io
2. ✅ Add `gateway_client` to AppState (keep `c2s_client`)
3. ✅ Update handlers one-by-one to use gateway
4. ✅ Test each handler after migration
5. ✅ Once all migrated, remove `c2s_client`

**Pros**: Safe, reversible, can test gradually
**Cons**: Temporary dual-client setup

### Option B: Complete Migration

1. ✅ Deploy C2S Gateway
2. ✅ Replace `c2s_client` with `gateway_client` entirely
3. ✅ Update all handlers at once
4. ✅ Test everything

**Pros**: Clean, no backward compatibility needed
**Cons**: Riskier, harder to rollback

---

## Gateway Deployment

### C2S Gateway Location
```
/Users/ronaldo/Documents/projects/clients/ibvi/services/ads/platform/c2s-gateway/
```

### Deploy to Fly.io

```bash
cd c2s-gateway

# Login to Fly.io
fly auth login

# Create app (first time only)
fly apps create c2s-gateway-mbras

# Set secrets
fly secrets set C2S_TOKEN="<your_token>"
fly secrets set C2S_BASE_URL="https://api.contact2sale.com"

# Deploy
fly deploy

# Get URL
fly status
# Example: https://c2s-gateway-mbras.fly.dev
```

### Verify Deployment

```bash
# Health check
curl https://c2s-gateway-mbras.fly.dev/

# Get lead
curl https://c2s-gateway-mbras.fly.dev/leads/bf1a88eaa4ab34b01a257536563fb42b
```

---

## Testing Plan

### 1. Gateway Health Check
```bash
curl https://c2s-gateway-mbras.fly.dev/
```

**Expected**: `{"status": "ok"}`

### 2. Get Lead via Gateway
```bash
curl https://c2s-gateway-mbras.fly.dev/leads/bf1a88eaa4ab34b01a257536563fb42b
```

**Expected**: Lead JSON data

### 3. Send Message via Gateway
```bash
curl -X POST https://c2s-gateway-mbras.fly.dev/leads/bf1a88eaa4ab34b01a257536563fb42b/messages \
  -H "Content-Type: application/json" \
  -d '{"message": "Test message from Rust API"}'
```

**Expected**: 200 OK

### 4. Full Integration Test
```bash
# Call Rust API enrichment endpoint (should use gateway internally)
curl -X POST https://mbras-c2s.fly.dev/api/v1/c2s/enrich/bf1a88eaa4ab34b01a257536563fb42b
```

**Expected**:
- ✅ Rust API fetches lead via gateway
- ✅ Enriches with Work API
- ✅ Sends enriched data back via gateway
- ✅ Returns success

---

## Code Changes Summary

### Files to Modify

1. **`src/services.rs`** - Add `C2sGatewayClient` struct
2. **`src/config.rs`** - Add `c2s_gateway_url` field
3. **`src/main.rs`** - Add `gateway_client` to `AppState`
4. **`src/handlers.rs`** - Update handlers to use gateway
5. **`.env.example`** - Add `C2S_GATEWAY_URL`
6. **`fly.toml`** - Add `C2S_GATEWAY_URL` secret (deployment)

### New Dependencies

**None!** Already using `reqwest` and `serde_json`.

---

## Rollback Plan

If integration fails:

1. **Immediate**: Remove gateway calls, use direct C2S client
2. **Environment**: Unset `C2S_GATEWAY_URL` to disable
3. **Code**: Keep `c2s_client` during migration for fallback
4. **Deploy**: Redeploy previous version

---

## Timeline

### Immediate (Today)
- ✅ Review this document
- ✅ Decide on migration strategy (A or B)
- 🔲 Deploy C2S Gateway to Fly.io

### Short Term (This Week)
- 🔲 Add `C2sGatewayClient` to Rust API
- 🔲 Update configuration and environment
- 🔲 Migrate one handler (e.g., `get_lead`)
- 🔲 Test integration

### Medium Term (Next Week)
- 🔲 Migrate all handlers to use gateway
- 🔲 Remove direct C2S client (if Option B)
- 🔲 Update documentation
- 🔲 Deploy to production

---

## Questions to Resolve

1. **Gateway URL**: What should the Fly.io app be named?
   - Suggestion: `c2s-gateway-mbras` or `mbras-c2s-gateway`

2. **Migration Strategy**: Gradual (A) or Complete (B)?
   - Recommendation: **Option A** (gradual) for safety

3. **Backward Compatibility**: Keep direct C2S client as fallback?
   - Recommendation: **Yes** during migration, remove after

4. **Error Handling**: How to handle gateway failures?
   - Suggestion: Retry logic with fallback to direct C2S

5. **Gateway Location**: Deploy as separate Fly.io app or use existing infrastructure?
   - Recommendation: **Separate Fly.io app** for independence

---

## Next Steps

1. **Review this plan** with team
2. **Deploy C2S Gateway** to Fly.io
3. **Test gateway** endpoints manually
4. **Start migration** with one handler
5. **Full migration** once tested
6. **Update CLAUDE.md** with new architecture

---

**Status**: 📋 Ready for implementation!  
**Estimated Effort**: 4-6 hours  
**Risk Level**: Low (can rollback easily)
