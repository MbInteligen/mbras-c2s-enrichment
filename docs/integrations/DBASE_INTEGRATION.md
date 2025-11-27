# DBase API Integration

**Date:** 2025-11-26  
**Status:** ✅ Deployed and Active  
**Version:** 35

---

## 📋 Overview

The DBase API integration provides a **fallback mechanism** for phone number lookups when the primary Diretrix API fails or returns no results. DBase Brasil operates one of the largest databases in Brazil with **1.2 billion phone numbers**, making it an excellent secondary data source for enrichment.

---

## 🎯 Purpose

**Primary Use Case:** Fallback for phone-based CPF lookups

When a lead or customer has a phone number but:
1. Diretrix API fails to find a match
2. Diretrix API is unavailable
3. Diretrix API times out

The system automatically attempts a DBase lookup to maximize data enrichment success rates.

---

## 🔑 Configuration

### Environment Variables

**Required:**
```bash
DBASE_KEY=your_dbase_api_key_here
```

**Deployment:**
```bash
# Set in Fly.io secrets
fly secrets set DBASE_KEY="your_api_key_here"
```

**Example (.env.example):**
```bash
# DBase API Configuration (fallback for Diretrix)
DBASE_KEY=your_dbase_api_key_here
```

### API Details

- **Base URL:** `https://app.dbase.com.br/sistema/consultas/Data-basebrasil-api2024/api`
- **Authentication:** Bearer token via `Authorization` header
- **Method:** POST with multipart form data
- **Data Coverage:** 220M CPFs, 72M CNPJs, 1.2B phone numbers

---

## 🏗️ Architecture

### Integration Flow

```
1. Lead Enrichment Starts
   ↓
2. Try Diretrix Phone Lookup
   ↓
3. Diretrix Failed/No Results?
   ↓ YES
4. Try DBase Phone Lookup (FALLBACK)
   ↓
5. Convert DBase → Diretrix Format
   ↓
6. Continue with Work API Enrichment
```

### Code Structure

**Models** (`src/models.rs`):
- `DBasePhoneResponse` - Main response structure
- `DBasePhone` - Phone number data
- `DBaseEmail` - Email address data
- `DBaseAddress` - Address information

**Service** (`src/services.rs`):
- `DBaseService::new()` - Initialize service
- `DBaseService::search_by_phone()` - Phone lookup
- `DBaseService::search_by_name()` - Name lookup (not currently used)

**Enrichment** (`src/enrichment.rs`):
- `find_cpf_via_diretrix()` - Main function with fallback logic

---

## 📊 Request/Response Format

### Request (Phone Lookup)

```http
POST https://app.dbase.com.br/sistema/consultas/Data-basebrasil-api2024/api
Authorization: Bearer your_api_key_here
Content-Type: multipart/form-data

telefone=11987654321
nome=
```

**Notes:**
- Phone number is normalized (digits only, no country code)
- `nome` field sent empty when searching by phone only
- System removes Brazil country code (55) if present

### Response

```json
{
  "nome": "MARIA SILVA SANTOS",
  "cpf": "12345678901",
  "dataNascimento": "15/03/1985",
  "idade": "39",
  "sexo": "F",
  "mae": "JOANA MARIA SILVA",
  "pai": "JOSE CARLOS SILVA",
  "rg": "123456789",
  "telefones": [
    {
      "numero": "11987654321",
      "ddd": "11",
      "operadora": "VIVO",
      "tipo": "Celular"
    }
  ],
  "emails": [
    {
      "email": "maria.silva@example.com"
    }
  ],
  "enderecos": [
    {
      "logradouro": "Rua das Flores",
      "numero": "123",
      "complemento": "Apto 45",
      "bairro": "Jardim Europa",
      "cidade": "São Paulo",
      "uf": "SP",
      "cep": "01234-567"
    }
  ]
}
```

---

## 🔄 Fallback Logic

### When Fallback Triggers

```rust
// In src/enrichment.rs:find_cpf_via_diretrix()

// 1. Try Diretrix first
let phone_lookup = diretrix_service.search_by_phone(phone).await.ok();

// 2. If Diretrix failed, try DBase
if phone_lookup.is_none() && validated_phone.is_some() {
    tracing::info!("Diretrix phone lookup failed, trying DBase fallback");
    
    match dbase_service.search_by_phone(phone).await {
        Ok(Some(dbase_data)) => {
            if let Some(cpf) = dbase_data.cpf {
                tracing::info!("✓ DBase fallback found CPF: {}", cpf);
                // Convert to Diretrix format for compatibility
                phone_lookup = Some(vec![DiretrixPersonSearch {
                    nome: dbase_data.nome.unwrap_or_default(),
                    cpf,
                }]);
            }
        }
        Ok(None) => tracing::info!("DBase fallback returned no data"),
        Err(e) => tracing::warn!("DBase fallback failed: {}", e),
    }
}
```

### Phone Normalization

**Input:** `+55 (11) 98765-4321` or `5511987654321` or `11987654321`  
**Normalized:** `11987654321` (digits only, no country code)

```rust
// Remove non-digits
let phone_clean: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();

// Remove Brazil country code (55) if present
let phone_normalized = if phone_clean.starts_with("55") && phone_clean.len() > 2 {
    &phone_clean[2..]
} else {
    &phone_clean
};
```

---

## 📈 Success Metrics

### Expected Behavior

**Scenario 1: Diretrix Success**
- Diretrix finds CPF → DBase NOT called
- Fastest response time
- Primary data source used

**Scenario 2: Diretrix Fails, DBase Success**
- Diretrix returns no results → DBase called
- DBase finds CPF → Enrichment continues
- **Increased success rate** (fallback working)

**Scenario 3: Both Fail**
- Both APIs return no results
- Error: "Could not find CPF via Diretrix"
- Manual review recommended

### Monitoring

**Log Messages:**
```
✓ DBase fallback found CPF: 12345678901       # Success
Diretrix phone lookup failed, trying DBase    # Fallback triggered
DBase fallback returned no data               # Fallback found nothing
DBase fallback failed: <error>                # Fallback errored
```

**Success Rate Formula:**
```
Total Success = (Diretrix Success + DBase Success) / Total Lookups
```

---

## 🛡️ Error Handling

### Graceful Degradation

DBase failures **do not break** the enrichment flow:

```rust
match dbase_service.search_by_phone(phone).await {
    Ok(Some(data)) => { /* Use data */ }
    Ok(None) => { /* Log and continue */ }
    Err(e) => { 
        tracing::warn!("DBase fallback failed: {}", e);
        // Continue with existing flow
    }
}
```

### Error Scenarios

| Error | Handling | Impact |
|-------|----------|--------|
| **Invalid API Key** | Returns `Ok(None)`, logs warning | Falls back to "not found" |
| **Network timeout** | Returns `Ok(None)`, logs warning | Falls back to "not found" |
| **Invalid phone format** | Normalized before request | Prevents API errors |
| **No CPF in response** | Returns `Ok(None)`, logs warning | Falls back to "not found" |
| **JSON parse error** | Returns `Ok(None)`, logs warning | Falls back to "not found" |

**Philosophy:** **Never fail enrichment due to DBase errors** - it's a bonus, not a requirement.

---

## 🔧 Dependencies

### Cargo.toml

```toml
reqwest = { version = "0.12", features = ["json", "multipart"] }
```

**Note:** `multipart` feature is required for DBase form-data requests.

---

## 📝 Implementation Details

### Files Modified

1. **`src/models.rs`**
   - Added `DBasePhoneResponse` struct
   - Added `DBasePhone`, `DBaseEmail`, `DBaseAddress` structs
   - Comprehensive doc comments with field descriptions

2. **`src/services.rs`**
   - Added `DBaseService` struct
   - Implemented `search_by_phone()` method
   - Implemented `search_by_name()` method (unused, for future use)
   - Added Bearer token authentication
   - Added phone normalization logic

3. **`src/enrichment.rs`**
   - Updated `find_cpf_via_diretrix()` to include DBase fallback
   - Added import: `use crate::services::DBaseService`
   - Added conversion logic: DBase → Diretrix format

4. **`src/config.rs`**
   - Added `dbase_key: String` field
   - Added validation for `DBASE_KEY` env var
   - Added configuration logging

5. **`Cargo.toml`**
   - Added `multipart` feature to reqwest dependency

6. **`.env.example`**
   - Added `DBASE_KEY` documentation

---

## 🧪 Testing

### Manual Testing

```bash
# Test phone lookup (requires valid DBASE_KEY)
curl -X POST "https://mbras-c2s.fly.dev/api/v1/c2s/enrich/LEAD_ID" \
  -H "Content-Type: application/json" \
  -d '{
    "phone": "11987654321",
    "email": null
  }'
```

**Expected:**
1. Diretrix lookup attempted first
2. If Diretrix fails, DBase lookup attempted
3. If DBase succeeds, CPF found and enrichment continues

### Log Verification

```bash
# Check Fly.io logs for DBase activity
fly logs

# Look for:
# - "Diretrix phone lookup failed, trying DBase fallback"
# - "✓ DBase fallback found CPF: ..."
# - "DBase: Searching by phone: ..."
```

---

## 📊 Data Quality

### DBase vs Diretrix

| Feature | Diretrix | DBase |
|---------|----------|-------|
| **Phone Coverage** | Good | Excellent (1.2B records) |
| **CPF Data** | Yes | Yes |
| **Address Data** | Detailed | Basic |
| **Update Frequency** | Unknown | Daily |
| **Response Time** | Fast | Medium |
| **Cost per Query** | Low | Medium |

### When to Use Which

**Use Diretrix (Primary):**
- Faster response time
- More detailed relationship data
- Lower cost per query

**Use DBase (Fallback):**
- Larger database coverage
- When Diretrix fails
- Newer phone numbers

---

## 🚀 Future Enhancements

### Potential Improvements

1. **Email Lookup Fallback**
   - Currently only phone has DBase fallback
   - Could add DBase email search when Diretrix email fails

2. **Caching DBase Results**
   - Similar to Work API caching
   - 1-hour TTL for repeated lookups
   - Reduce API costs

3. **Parallel Lookups**
   - Call Diretrix and DBase simultaneously
   - Use fastest response
   - Merge results for higher confidence

4. **Response Enrichment**
   - Merge DBase address data with Diretrix results
   - Combine phone lists from both sources
   - Cross-validate CPF matches

5. **Metrics Dashboard**
   - Track fallback success rate
   - Monitor DBase API health
   - Compare Diretrix vs DBase data quality

---

## 📚 API Documentation

### Official Links

- **DBase Website:** https://www.dbase.com.br/
- **Data Coverage:** 220M CPFs, 72M CNPJs, 1.2B phones
- **API Endpoint:** (authenticated access only)

### Support

For DBase API issues:
- Contact DBase support via their website
- Check API status (no public status page)
- Verify API key validity

---

## ✅ Deployment Checklist

- [x] Add `DBASE_KEY` to `.env.example`
- [x] Update `src/config.rs` with new field
- [x] Create `DBaseService` in `src/services.rs`
- [x] Add models to `src/models.rs`
- [x] Integrate fallback in `src/enrichment.rs`
- [x] Add `multipart` feature to Cargo.toml
- [x] Deploy `DBASE_KEY` secret to Fly.io
- [x] Test compilation (`cargo check`)
- [x] Deploy to production (`fly deploy`)
- [x] Verify logs for DBase activity
- [x] Document integration (this file)

---

## 🔐 Security Notes

### API Key Protection

**NEVER commit the actual API key:**

```bash
# ❌ WRONG
DBASE_KEY=07AF4C33-13B7-4D71-B323-40E20F0A3FEE

# ✅ CORRECT
DBASE_KEY=your_dbase_api_key_here
```

**Key Storage:**
- Production: Fly.io secrets (`fly secrets set DBASE_KEY="..."`)
- Local: `.env` file (gitignored)
- Documentation: Placeholders only

**Key Rotation:**
If key is compromised:
1. Generate new key from DBase dashboard
2. Update Fly.io secret: `fly secrets set DBASE_KEY="new_key"`
3. Verify deployment health

---

## 📞 Contact

**Integration Maintainer:** MbInteligen Team  
**Deployment Date:** November 26, 2025  
**Version:** 35  
**Status:** ✅ Production Ready

---

**Last Updated:** 2025-11-26  
**Deployment URL:** https://mbras-c2s.fly.dev
