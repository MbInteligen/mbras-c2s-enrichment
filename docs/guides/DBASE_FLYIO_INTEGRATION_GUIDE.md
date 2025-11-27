# Complete Guide: DBase API Integration with Fly.io Static Egress IP

**Date:** November 27, 2025  
**Author:** Claude AI + Ronaldo  
**Purpose:** Step-by-step guide to integrate DBase API in any Rust/Axum application with Fly.io static egress IP

---

## 📋 Table of Contents

1. [Overview](#overview)
2. [Prerequisites](#prerequisites)
3. [Step 1: Allocate Static Egress IP on Fly.io](#step-1-allocate-static-egress-ip-on-flyio)
4. [Step 2: Whitelist IP with DBase](#step-2-whitelist-ip-with-dbase)
5. [Step 3: Add DBase Dependencies](#step-3-add-dbase-dependencies)
6. [Step 4: Configure Environment Variables](#step-4-configure-environment-variables)
7. [Step 5: Create DBase Data Models](#step-5-create-dbase-data-models)
8. [Step 6: Implement DBase Service](#step-6-implement-dbase-service)
9. [Step 7: Integrate with Existing Lookup Flow](#step-7-integrate-with-existing-lookup-flow)
10. [Step 8: Deploy and Test](#step-8-deploy-and-test)
11. [Troubleshooting](#troubleshooting)

---

## Overview

This guide shows how to integrate DBase API as a data lookup service with a **static egress IP** to avoid IP whitelisting issues on Fly.io.

**Why you need this:**
- DBase requires IP whitelisting for API access
- Fly.io uses dynamic outbound IPs by default (change on redeploy)
- Static egress IPs ensure consistent whitelisting

**What you'll build:**
- DBase API client in Rust
- Phone number lookup with fallback to secondary service
- Automatic handling of IP whitelisting

---

## Prerequisites

- Rust project using Axum (or similar web framework)
- Deployed on Fly.io
- DBase API account and token
- `flyctl` CLI installed

---

## Step 1: Allocate Static Egress IP on Fly.io

### 1.1 Check Current Machine ID

```bash
fly machine list --app your-app-name
```

Output:
```
ID            	STATE  	REGION
56837939a60708	started	gru
```

Copy the Machine ID (e.g., `56837939a60708`).

### 1.2 Allocate Static Egress IPs

```bash
fly machine egress-ip allocate <machine-id> --app your-app-name -y
```

Example:
```bash
fly machine egress-ip allocate 56837939a60708 --app mbras-c2s -y
```

Output:
```
Allocated egress IPs for machine 56837939a60708:
IPv4: 209.71.78.135
IPv6: 2a09:8280:e615::b0:e53:0
```

**✅ Save these IPs** - they are now **static** and won't change.

### 1.3 Verify Allocation

```bash
fly machine egress-ip list --app your-app-name
```

Output:
```
MACHINE ID    	REGION	TYPE	EGRESS IP
56837939a60708	gru   	v4  	209.71.78.135
56837939a60708	gru   	v6  	2a09:8280:e615::b0:e53:0
```

### 1.4 Restart Machine (Important!)

The machine must restart to use the new egress IPs:

```bash
fly machine restart <machine-id> --app your-app-name
```

---

## Step 2: Whitelist IP with DBase

### 2.1 Contact DBase Support

Send an email to DBase support with:

```
Subject: IP Whitelist Request - API Access

Hello,

I need to whitelist the following IP addresses for API access:

IPv4: 209.71.78.135
IPv6: 2a09:8280:e615::b0:e53:0 (optional - if IPv6 support needed)

Account: [Your DBase Account Name/ID]
API Token: [Your Token - last 4 chars only]

Thank you!
```

### 2.2 Wait for Confirmation

DBase typically responds within 24-48 hours. They will confirm when the IPs are whitelisted.

### 2.3 Test Access (After Whitelisting)

```bash
curl -X POST "https://app.dbase.com.br/sistema/consultas/Data-basebrasil-api2024/" \
  -F "consulta=telefone" \
  -F "telefone=11999999999" \
  -F "token=YOUR_DBASE_TOKEN"
```

Success response:
```json
{
  "status": true,
  "data": { ... }
}
```

Failure response (not whitelisted):
```json
{
  "status": false,
  "msg": "003 - Seu ip não está liberado para acesso a api, informe o Ipv4/6: X.X.X.X"
}
```

---

## Step 3: Add DBase Dependencies

### 3.1 Update `Cargo.toml`

No new dependencies needed! You already have:

```toml
[dependencies]
reqwest = { version = "0.12", features = ["json", "multipart"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
tracing = "0.1"
```

---

## Step 4: Configure Environment Variables

### 4.1 Add to `.env.example`

```bash
# DBase API (Brazilian data lookup)
DBASE_KEY=your_dbase_token_here
```

### 4.2 Add to `.env` (Local Development)

```bash
DBASE_KEY=your_actual_token_here
```

### 4.3 Set Fly.io Secret (Production)

```bash
fly secrets set DBASE_KEY="your_actual_token_here" --app your-app-name
```

### 4.4 Update Config Struct

In `src/config.rs`:

```rust
#[derive(Clone)]
pub struct Config {
    // ... existing fields
    pub dbase_key: String,
}

impl Config {
    pub fn from_env() -> Result<Self, std::env::VarError> {
        Ok(Config {
            // ... existing fields
            dbase_key: std::env::var("DBASE_KEY")?,
        })
    }
}
```

---

## Step 5: Create DBase Data Models

### 5.1 Add to `src/models.rs`

```rust
use serde::{Deserialize, Serialize};

/// Response from DBase phone lookup API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DBasePhoneResponse {
    pub status: bool,
    pub msg: Option<String>,
    pub cpf: Option<String>,
    pub nome: Option<String>,
    pub data_nascimento: Option<String>,
    pub sexo: Option<String>,
    pub mae: Option<String>,
    pub situacao_cpf: Option<String>,
}

/// Standard person search result (compatible with Diretrix format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonSearch {
    pub nome: String,
    pub cpf: String,
}
```

---

## Step 6: Implement DBase Service

### 6.1 Create DBase Service Struct

Add to `src/services.rs`:

```rust
use crate::config::Config;
use crate::errors::AppError;
use crate::models::DBasePhoneResponse;
use reqwest::Client;

pub struct DBaseService {
    client: Client,
    base_url: String,
    api_key: String,
}

impl DBaseService {
    pub fn new(config: &Config) -> Self {
        Self {
            client: Client::new(),
            // Note: URL must have trailing slash per DBase API spec
            base_url: "https://app.dbase.com.br/sistema/consultas/Data-basebrasil-api2024/"
                .to_string(),
            api_key: config.dbase_key.clone(),
        }
    }

    /// Search person by phone number using DBase API
    ///
    /// # Arguments
    /// * `phone` - Phone number to search (can include country code, will be normalized)
    ///
    /// # Returns
    /// * `Ok(Some(DBasePhoneResponse))` - Person data found
    /// * `Ok(None)` - No data found or API error
    /// * `Err(AppError)` - Request failed
    ///
    /// # Example
    /// ```rust
    /// let result = dbase_service.search_by_phone("+5511999999999").await?;
    /// if let Some(data) = result {
    ///     println!("Found CPF: {}", data.cpf.unwrap_or_default());
    /// }
    /// ```
    pub async fn search_by_phone(
        &self,
        phone: &str,
    ) -> Result<Option<DBasePhoneResponse>, AppError> {
        // Normalize phone number - remove non-digits
        let phone_clean: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();

        // Remove country code if present (55 for Brazil)
        let phone_normalized = if phone_clean.starts_with("55") && phone_clean.len() > 2 {
            &phone_clean[2..]
        } else {
            &phone_clean
        };

        tracing::info!(
            "DBase: Searching by phone: {} (normalized: {})",
            phone,
            phone_normalized
        );

        // Build form data - DBase API expects multipart form-data with:
        // - consulta: type of query ("telefone")
        // - telefone: the phone number
        // - token: API key (NOT Bearer auth header)
        let form = reqwest::multipart::Form::new()
            .text("consulta", "telefone")
            .text("telefone", phone_normalized.to_string())
            .text("token", self.api_key.clone());

        let response = self
            .client
            .post(&self.base_url)
            .multipart(form)
            .send()
            .await
            .map_err(|e| {
                tracing::warn!("DBase API request failed: {}", e);
                AppError::ExternalApiError(format!("DBase phone search failed: {}", e))
            })?;

        let status = response.status();
        let response_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Failed to get response text".to_string());

        tracing::info!(
            "DBase API response status: {}, body: {}",
            status,
            &response_text[..response_text.len().min(500)]
        );

        if !status.is_success() {
            tracing::warn!(
                "DBase API returned error status {}: {}",
                status,
                response_text
            );
            return Ok(None);
        }

        // Parse JSON response
        match serde_json::from_str::<DBasePhoneResponse>(&response_text) {
            Ok(data) => {
                if data.status && data.cpf.is_some() {
                    tracing::info!("✓ DBase found person: CPF={}", data.cpf.as_ref().unwrap());
                    Ok(Some(data))
                } else {
                    tracing::info!("DBase: No person found for phone {}", phone);
                    Ok(None)
                }
            }
            Err(e) => {
                tracing::warn!("Failed to parse DBase response: {}", e);
                Ok(None)
            }
        }
    }
}
```

---

## Step 7: Integrate with Existing Lookup Flow

### 7.1 Add DBase to State

In `src/main.rs` or where you create your app state:

```rust
use crate::services::DBaseService;

pub struct AppState {
    pub db: PgPool,
    // ... other fields
    pub dbase_service: Arc<DBaseService>,
}

// In your main function:
let config = Config::from_env()?;
let dbase_service = Arc::new(DBaseService::new(&config));

let state = Arc::new(AppState {
    db: pool,
    // ... other fields
    dbase_service,
});
```

### 7.2 Create Lookup Function with Fallback Pattern

Create a new file `src/enrichment.rs` or add to existing services:

```rust
use crate::services::{DBaseService, DiretrixService, PersonSearch};
use std::sync::Arc;

/// Find CPF via phone lookup with DBase as primary, Diretrix as fallback
pub async fn find_cpf_by_phone(
    phone: &str,
    dbase_service: &Arc<DBaseService>,
    diretrix_service: &Arc<DiretrixService>,
) -> Option<Vec<PersonSearch>> {
    tracing::info!("Step 1: Trying DBase first for phone lookup");

    // Try DBase first (primary source)
    match dbase_service.search_by_phone(phone).await {
        Ok(Some(dbase_data)) => {
            if let Some(cpf) = dbase_data.cpf {
                tracing::info!("✓ DBase found CPF: {}", cpf);
                // Convert DBase response to standard PersonSearch format
                return Some(vec![PersonSearch {
                    nome: dbase_data.nome.unwrap_or_default(),
                    cpf,
                }]);
            } else {
                tracing::info!("DBase returned data but no CPF, will try Diretrix");
            }
        }
        Ok(None) => {
            tracing::info!("DBase returned no data, will try Diretrix");
        }
        Err(e) => {
            tracing::warn!("DBase lookup failed: {}, will try Diretrix", e);
        }
    }

    // If DBase didn't find CPF, try Diretrix as fallback
    tracing::info!("Step 2: DBase phone lookup failed/empty, trying Diretrix fallback");
    match diretrix_service.search_by_phone(phone).await {
        Ok(results) if !results.is_empty() => {
            tracing::info!("✓ Diretrix fallback found {} result(s)", results.len());
            Some(results)
        }
        Ok(_) => {
            tracing::info!("Diretrix fallback returned no results");
            None
        }
        Err(e) => {
            tracing::warn!("Diretrix fallback also failed: {}", e);
            None
        }
    }
}
```

### 7.3 Use in Handler

In your handler (e.g., `src/handlers.rs`):

```rust
use crate::enrichment::find_cpf_by_phone;

pub async fn enrich_lead_handler(
    State(state): State<Arc<AppState>>,
    Path(lead_id): Path<String>,
) -> Result<Json<Value>, AppError> {
    // Fetch lead from CRM
    let lead = fetch_lead_from_crm(&lead_id).await?;
    
    // Try to find CPF using phone (DBase → Diretrix fallback)
    let phone_lookup = if let Some(phone) = &lead.phone {
        find_cpf_by_phone(
            phone,
            &state.dbase_service,
            &state.diretrix_service,
        ).await
    } else {
        None
    };
    
    // Extract CPF from results
    let cpf = phone_lookup
        .and_then(|results| results.first().map(|r| r.cpf.clone()));
    
    if let Some(cpf) = cpf {
        // Proceed with enrichment using the found CPF
        tracing::info!("Found CPF: {}, enriching...", cpf);
        // ... rest of your enrichment logic
    } else {
        tracing::error!("Could not find CPF from phone");
        return Err(AppError::NotFound("CPF not found".into()));
    }
    
    Ok(Json(json!({"success": true})))
}
```

---

## Step 8: Deploy and Test

### 8.1 Compile and Check

```bash
cargo check
```

### 8.2 Deploy to Fly.io

```bash
fly deploy --strategy rolling
```

### 8.3 Check Deployment

```bash
fly status --app your-app-name
```

### 8.4 Test the Endpoint

```bash
# Test with a real phone number
curl "https://your-app.fly.dev/api/v1/enrich?phone=11999999999"
```

### 8.5 Monitor Logs

```bash
fly logs --app your-app-name
```

Look for:
```
✓ DBase found CPF: 12345678901
```

Or fallback:
```
DBase returned no data, will try Diretrix
✓ Diretrix fallback found 1 result(s)
```

---

## Troubleshooting

### Issue 1: IP Not Whitelisted Error

**Symptom:**
```json
{
  "status": false,
  "msg": "003 - Seu ip não está liberado para acesso a api, informe o Ipv4/6: X.X.X.X"
}
```

**Solutions:**

1. **Check the IP being used:**
   ```bash
   fly logs --app your-app-name | grep "informe o Ipv4"
   ```

2. **Verify static egress IP is allocated:**
   ```bash
   fly machine egress-ip list --app your-app-name
   ```

3. **Confirm IP matches what DBase has whitelisted:**
   - Contact DBase support to confirm
   - Provide the exact IP from logs

4. **Restart machine after allocation:**
   ```bash
   fly machine restart <machine-id> --app your-app-name
   ```

5. **Check if IPv6 is being used instead of IPv4:**
   - DBase may have only whitelisted IPv4
   - Solution: Drop IPv6 from whitelist or ask DBase to whitelist both

### Issue 2: Different IP After Redeploy

**Symptom:** IP changes after `fly deploy`

**Cause:** Static egress IP is tied to the **machine**, not the app. If Fly.io creates a new machine during deployment, the old machine's egress IP is lost.

**Solution:**

1. **Check if new machine was created:**
   ```bash
   fly machine list --app your-app-name
   ```

2. **Allocate egress IP for new machine:**
   ```bash
   fly machine egress-ip allocate <new-machine-id> --app your-app-name -y
   ```

3. **Whitelist the new IP with DBase**

4. **To prevent this:** Use `--strategy rolling` which updates existing machines instead of creating new ones:
   ```bash
   fly deploy --strategy rolling
   ```

### Issue 3: Request Timeout

**Symptom:** DBase requests timeout after 30s

**Cause:** DBase API can be slow for certain queries

**Solution:** Increase request timeout:

```rust
let client = Client::builder()
    .timeout(Duration::from_secs(60))  // 60 second timeout
    .build()?;
```

### Issue 4: Form Data Not Recognized

**Symptom:** DBase returns error about missing parameters

**Cause:** DBase API requires **multipart/form-data**, not JSON

**Solution:** Ensure you're using `reqwest::multipart::Form`:

```rust
let form = reqwest::multipart::Form::new()
    .text("consulta", "telefone")
    .text("telefone", phone)
    .text("token", api_key);

client.post(url).multipart(form).send().await?;
```

**NOT this:**
```rust
// ❌ Wrong - DBase doesn't accept JSON
client.post(url).json(&payload).send().await?;
```

### Issue 5: CPF Format Issues

**Symptom:** DBase returns data but CPF is formatted/invalid

**Solution:** Always clean the CPF:

```rust
let cpf_clean: String = cpf.chars().filter(|c| c.is_ascii_digit()).collect();
assert_eq!(cpf_clean.len(), 11); // Brazilian CPF is always 11 digits
```

---

## Best Practices

### 1. Always Use Fallback Pattern

Don't rely on a single data source. Always have a fallback:

```rust
// ✅ Good: Primary + Fallback
let result = try_dbase().await
    .or_else(|_| try_diretrix().await)
    .or_else(|_| try_database_cache().await);

// ❌ Bad: Single point of failure
let result = try_dbase().await?;
```

### 2. Log Everything

Add detailed logging for debugging:

```rust
tracing::info!("Step 1: Trying DBase");
tracing::info!("✓ DBase found data: {:?}", data);
tracing::warn!("DBase failed: {}, trying fallback", error);
```

### 3. Cache Results

Implement caching to reduce API calls:

```rust
// Check cache first
if let Some(cached) = cache.get(&phone).await {
    return Ok(cached);
}

// Call API
let result = dbase_service.search_by_phone(phone).await?;

// Store in cache (1 hour TTL)
cache.insert(phone, result.clone(), Duration::from_secs(3600)).await;
```

### 4. Handle Rate Limits

Add delays between requests if needed:

```rust
use tokio::time::{sleep, Duration};

sleep(Duration::from_millis(500)).await; // 500ms delay
let result = dbase_service.search_by_phone(phone).await?;
```

### 5. Monitor Costs

Track API usage to monitor costs:

```rust
tracing::info!(
    target: "metrics",
    event = "dbase_api_call",
    phone = phone,
    success = result.is_some()
);
```

---

## Summary Checklist

- [ ] Allocate static egress IP on Fly.io
- [ ] Whitelist IP with DBase support
- [ ] Add `DBASE_KEY` to environment variables
- [ ] Implement DBaseService struct
- [ ] Create data models (DBasePhoneResponse)
- [ ] Add lookup function with fallback pattern
- [ ] Integrate with existing handlers
- [ ] Deploy to Fly.io
- [ ] Test with real phone numbers
- [ ] Monitor logs for errors
- [ ] Verify IP doesn't change after redeploy

---

## Cost Considerations

**Fly.io Static Egress IP:**
- **Cost:** ~$2/month per IP
- **Worth it?** Yes, for production APIs with IP whitelisting requirements

**DBase API:**
- Check with DBase for current pricing
- Consider implementing caching to reduce calls

---

## Example Project Structure

```
your-api/
├── src/
│   ├── main.rs              # App initialization, add DBaseService to state
│   ├── config.rs            # Add dbase_key field
│   ├── models.rs            # Add DBasePhoneResponse
│   ├── services.rs          # Add DBaseService implementation
│   ├── enrichment.rs        # Add find_cpf_by_phone with fallback
│   ├── handlers.rs          # Use enrichment functions
│   └── errors.rs            # Existing error types
├── Cargo.toml               # Already has required dependencies
├── .env.example             # Add DBASE_KEY placeholder
├── .env                     # Add actual DBASE_KEY (gitignored)
└── docs/
    └── DBASE_INTEGRATION.md # This guide!
```

---

## Additional Resources

- **DBase API Documentation:** Contact DBase support for swagger/OpenAPI spec
- **Fly.io Egress IPs:** https://fly.io/docs/networking/egress-ips/
- **Reqwest Documentation:** https://docs.rs/reqwest/
- **Axum Documentation:** https://docs.rs/axum/

---

**Questions?** Add them as GitHub issues or contact the development team.

**Version:** 1.0  
**Last Updated:** November 27, 2025
