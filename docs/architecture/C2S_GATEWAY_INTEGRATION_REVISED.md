# C2S Gateway Integration Plan (Revised)

**Date**: 2025-11-20  
**Status**: Ready for implementation  
**Priority**: Medium (direct C2S path works today; gateway adds coverage and maintainability)

---

## Executive Summary

Two C2S-facing services need to align:

1. **Rust C2S API** (this project) enriches leads with Work API data.
2. **Python C2S Gateway** is the full C2S wrapper with 28+ endpoints and campaign enrichment.

Recommendation: keep services separate but route C2S traffic from Rust through the Python gateway. This centralizes C2S auth/changes and unlocks additional endpoints without bloating Rust.

Definition of done (minimum): gateway deployed, `C2S_GATEWAY_URL` configured, one Rust endpoint calling the gateway, logs show requests hitting the gateway.

---

## Why Integrate?

- Duplicate C2S auth and token handling in two codebases.
- Each C2S API change requires two updates.
- Rust API lacks campaign enrichment and is limited to basic operations.
- Gateway already implements retries, validation, and enrichment.
- Single integration point simplifies maintenance and onboarding.

---

## Architecture

### Before
```
Make.com -> Rust API -> Work API (enrichment)
                  \
                   -> C2S API (direct)
```

### After
```
Make.com -> Rust API -> Work API (enrichment)
                  \
                   -> Python Gateway -> C2S API
```

---

## Decision Guide

- Choose **Option A: Full Integration** if you want all C2S traffic centralized now (remove C2S creds from Rust, 2-3 hours).
- Choose **Option B: Partial Integration** if you prefer a low-risk pilot (keep direct calls, route new features through gateway first, ~1 hour).
- **Option C: No Integration** if current scope is enough and time is very limited.

---

## Implementation Plan (Option B as default)

1) Deploy and verify gateway (30 min)  
```bash
cd /Users/ronaldo/Documents/projects/clients/ibvi/services/ads/platform/c2s-gateway
fly deploy
fly status  # capture https://c2s-gateway.fly.dev
curl https://c2s-gateway.fly.dev/
```

2) Configure Rust API (5 min)  
Add gateway URL to `.env` (or Fly secrets):  
```bash
echo "C2S_GATEWAY_URL=https://c2s-gateway.fly.dev" >> .env
```

3) Add gateway client to Rust (1 hour)  
Create `src/gateway_client.rs`:
```rust
use reqwest;
use serde_json::json;
use std::time::Duration;
use crate::errors::AppError;

pub struct GatewayClient {
    client: reqwest::Client,
    base_url: String,
}

impl GatewayClient {
    pub fn new(base_url: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap();
        Self { client, base_url }
    }

    pub async fn get_lead(&self, lead_id: &str) -> Result<serde_json::Value, AppError> {
        let url = format!("{}/leads/{}", self.base_url, lead_id);
        let response = self.client.get(&url).send().await?.json().await?;
        Ok(response)
    }

    pub async fn send_message(&self, lead_id: &str, message: &str) -> Result<(), AppError> {
        let url = format!("{}/leads/{}/messages", self.base_url, lead_id);
        self.client.post(&url).json(&json!({ "message": message })).send().await?;
        Ok(())
    }
}
```

4) Wire handlers to gateway (30 min)  
```rust
// before
let lead = c2s_client.get_lead(&lead_id).await?;
c2s_client.send_message(&lead_id, &message).await?;

// after
let lead = gateway_client.get_lead(&lead_id).await?;
gateway_client.send_message(&lead_id, &message).await?;
```

5) Test locally (30 min)  
```bash
cargo run
curl -X POST http://localhost:8080/api/v1/c2s/enrich/LEAD_ID
```
Check logs for gateway URL hits.

6) Deploy Rust API and retest (20 min)  
```bash
fly deploy
curl -X POST https://mbras-c2s.fly.dev/api/v1/c2s/enrich/LEAD_ID
```

---

## What Each Service Covers

- **Python Gateway**: all C2S operations (create/update/get leads, messages, activities, tags, sellers, queues, campaign enrichment, webhooks) with retries/validation.
- **Rust C2S API**: enrichment (Diretrix CPF lookup, Work API person data, address confidence scoring, PostgreSQL persistence, send enriched payloads back to C2S via gateway).

---

## Files to Touch

- `.env` (add `C2S_GATEWAY_URL`)
- `src/gateway_client.rs` (new)
- `src/main.rs` (inject gateway client into state)
- `src/handlers.rs` (swap direct C2S calls for gateway in at least one endpoint)

For full migration also remove C2S credentials and client code from `src/services.rs` and `src/config.rs`.

---

## Risks and Mitigations

- Gateway unavailable -> keep direct C2S as temporary fallback until stable.
- Gateway latency -> confirm Fly region, keep 30s timeout in client.
- Gateway bugs -> start with a single endpoint, monitor logs, roll back by switching handler back to direct client.
- Auth/secret drift -> move C2S credentials entirely to gateway once Option A is done.

---

## Quick Win (smoke test)

Add a temporary endpoint to prove connectivity:
```rust
// in src/handlers.rs
pub async fn test_gateway(State(_state): State<AppState>) -> Result<String, AppError> {
    let gateway_url = "https://c2s-gateway.fly.dev";
    let response = reqwest::get(format!("{}/leads", gateway_url)).await?.text().await?;
    Ok(format!("Gateway response: {}", response))
}
```
Call with `curl http://localhost:8080/test-gateway`. If good, migrate one real endpoint next.

---

## Commands Cheatsheet

```bash
# Deploy Python Gateway
cd /Users/ronaldo/Documents/projects/clients/ibvi/services/ads/platform/c2s-gateway
fly deploy

# Test gateway
curl https://c2s-gateway.fly.dev/leads

# Run Rust API locally
cd /Users/ronaldo/Documents/projects/clients/mbras/tools/c2s/rust-c2s-api
cargo run

# Test integration
curl -X POST http://localhost:8080/api/v1/c2s/enrich/LEAD_ID

# Deploy Rust API
fly deploy

# Logs
fly logs -a mbras-c2s
fly logs -a c2s-gateway
```

---

Ready to start: deploy the Python gateway, set `C2S_GATEWAY_URL`, and reroute one endpoint through it. That validates the path before full migration.
