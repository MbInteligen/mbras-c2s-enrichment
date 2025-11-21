# CLAUDE.md - Session Context for AI Assistant

> **Purpose**: This file provides essential context for Claude (or any AI assistant) to quickly understand the project structure, conventions, and key information for productive coding sessions.

---

## Project Overview

**Name**: `rust-c2s-api`  
**Type**: Rust REST API for lead enrichment and C2S integration  
**Primary Function**: Enrich customer/lead data using Work API and Diretrix, then send enriched data to Contact2Sale (C2S)

**Tech Stack**:
- Language: Rust (Edition 2024, requires nightly toolchain)
- Web Framework: Axum
- Database: PostgreSQL (Neon.tech hosted)
- ORM: SQLx (async)
- HTTP Client: Reqwest
- Deployment: Fly.io (256MB instance, São Paulo region)

**Related Project**: 
- **C2S Gateway** (Python/FastAPI) - Located at `/Users/ronaldo/Documents/projects/clients/ibvi/services/ads/platform/c2s-gateway/`
  - Complete C2S API wrapper with 28+ endpoints
  - Campaign enrichment system
  - Can be integrated with this Rust API (see `docs/architecture/C2S_GATEWAY_INTEGRATION.md`)

---

## Project Structure

```
rust-c2s-api/
├── src/                      # Source code
│   ├── main.rs              # Entry point, routes
│   ├── handlers.rs          # API endpoint handlers
│   ├── services.rs          # External API services (Work API, C2S, Diretrix)
│   ├── models.rs            # Data models & DTOs
│   ├── config.rs            # Configuration management
│   ├── db.rs                # Database connection
│   ├── db_storage.rs        # Data persistence logic
│   └── errors.rs            # Error handling
│
├── examples/                 # Rust examples
│   ├── batch_enrich.rs      # Batch CPF enrichment (direct Work API)
│   └── import_json_to_db.rs # Import enriched JSON to PostgreSQL
│
├── scripts/                  # Bash scripts & utilities
│   ├── enrich_batch.sh      # Batch enrichment via API endpoint
│   ├── retry_failed_cpfs.sh # Retry failed enrichments
│   ├── import_enriched_to_db.sh  # Import via psql
│   ├── RUN_SERVER.sh        # Start the server locally
│   ├── TEST_LIVE.sh         # Test live deployment
│   ├── test-docker.sh       # Test with Docker
│   ├── test-local.sh        # Test local environment
│   ├── test_all_modules.sh  # Test all Work API modules
│   ├── test_concurrent_requests.sh  # Concurrency testing
│   ├── test_direct_work_api.sh      # Test Work API directly
│   ├── test_modules.sh      # Test specific modules
│   └── test_work_api.sh     # Test Work API integration
│
├── docs/                     # Documentation
│   ├── analysis/            # Data analysis and comparisons
│   │   ├── DATA_COMPARISON.md
│   │   └── DB_STORAGE_ANALYSIS.md
│   ├── architecture/        # System architecture and design
│   │   ├── DEDUPLICATION_IMPLEMENTATION.md
│   │   ├── IMPLEMENTATION_SUMMARY.md
│   │   └── PLAN_WEBHOOK_REDIS.md
│   ├── deployment/          # Deployment guides and checklists
│   │   ├── DEPLOYMENT.md
│   │   ├── DEPLOYMENT_CHECKLIST.md
│   │   ├── DEPLOY_NOW.md
│   │   ├── FLY_DEPLOYMENT.md
│   │   └── READY_FOR_DEPLOYMENT.md
│   ├── examples/            # Example API responses and data
│   │   ├── EXAMPLE_CPF_RESPONSE.json
│   │   └── WEALTH_ASSESSMENT_EXAMPLE.json
│   ├── integrations/        # External API integration docs
│   │   ├── MAKE_INTEGRATION.md
│   │   ├── MODULE_TEST_RESULTS.md
│   │   ├── TESTING.md
│   │   ├── TESTING_COMPLETE.md
│   │   └── WORK_API_RATE_LIMITING.md
│   ├── performance/         # Performance monitoring and reports
│   │   ├── MEMORY_USAGE_REPORT.md
│   │   └── PERFORMANCE_MONITORING.md
│   ├── queries/             # SQL query examples
│   │   └── ENRICHMENT_FLOW.md
│   ├── schemas/             # Database schema files
│   ├── security/            # Security checklists and guides
│   │   ├── SECURITY_AND_SCHEMA_FIXES.md
│   │   ├── SECURITY_CHECKLIST.md
│   │   ├── SECURITY_FIXES.md
│   │   └── SECURITY_ROTATION_REQUIRED.md
│   ├── sessions/            # Development session summaries
│   │   ├── FINAL_STATUS.md
│   │   ├── IMPLEMENTATION_COMPLETE.md
│   │   ├── PROJECT_SUMMARY.md
│   │   ├── QUICKSTART.md
│   │   └── SESSION_SUMMARY.md
│   ├── API_ENDPOINTS.md     # API endpoint documentation
│   └── README.md            # Documentation index
│
├── tests/                    # Integration tests (JS)
├── temp_data/               # Temporary files (gitignored)
├── migrations/              # (planned) SQL migrations
│
├── Cargo.toml               # Rust dependencies
├── Dockerfile               # Multi-stage Docker build (nightly Rust)
├── fly.toml                 # Fly.io configuration
├── docker-compose.yml       # Local development
├── .env.example             # Environment variable template
└── README.md                # Project documentation
```

---

## Environment Variables

**Required** (stored in `.env`, **never commit**):

```bash
# Database
DB_URL=postgresql://user:pass@host/db?sslmode=require

# Work API (enrichment data provider)
WORK_API=<api_key_here>

# C2S (Contact2Sale) Integration
C2S_TOKEN=<token_here>
C2S_BASE_URL=https://api.contact2sale.com

# Diretrix (CPF lookup service)
DIRETRIX_BASE_URL=http://api.diretrixconsultoria.com.br
DIRETRIX_USER=100198
DIRETRIX_PASS=<password_here>

# Server
PORT=8080
```

**Template**: See `.env.example` for reference

---

## Key API Endpoints

### Health Check
- **GET** `/health` - Returns service health status

### Customer Data
- **GET** `/api/v1/contributor/customer?cpf=XXX` - Get enriched customer data
- **GET** `/api/v1/customers/:id` - Get customer by UUID
- **POST** `/api/v1/enrich` - Enrich customer data

### Work API Integration
- **GET** `/api/v1/work/modules/all?documento=<cpf>` - Fetch all Work API modules
- **GET** `/api/v1/work/modules/:module?documento=<cpf>` - Fetch specific module
- **GET** `/api/v1/work/modules/cep?documento=<cep>` - Lookup people by CEP (returns list)

### Lead Processing
- **POST** `/api/v1/leads` - Process lead (basic)
- **POST** `/api/v1/c2s/enrich/:lead_id` - Complete C2S enrichment flow
- **GET** `/api/v1/leads/process?id=<lead_id>` - Trigger enrichment (Make.com integration)

---

## Important Conventions & Gotchas

### 1. Work API Rate Limiting
- **Recommended delay**: **3 seconds** between requests
- See `docs/integrations/WORK_API_RATE_LIMITING.md` for details
- Failures are usually timeouts, not rate limits
- Use retry logic with exponential backoff (5s, 10s, 20s)

### 2. Data Format Conversions

**Dates**:
- Work API returns: `DD/MM/YYYY`
- PostgreSQL expects: `YYYY-MM-DD`
- **Convert**: `split('/') → format!("{}-{}-{}", parts[2], parts[1], parts[0])`

**Sex/Gender**:
- Work API returns: `"M - MASCULINO"` or `"F - FEMININO"`
- Database expects: `CHAR(1)` → `'M'` or `'F'`
- **Convert**: Take first character only

**CPF**:
- Always 11 digits
- May come with or without formatting (dots/dashes)
- Store as plain text without formatting

### 3. Database Schema

**Core Tables**:
- `core.parties` - People (customers/leads)
  - NO unique constraint on `cpf_cnpj` (allows duplicates)
  - `enriched` boolean flag for enriched records
  
- `app.emails` - Email addresses
  - UNIQUE constraint on `normalized_email` (auto-generated lowercase/trimmed)
  - NO unique constraint on `email` field itself
  
- `app.phones` - Phone numbers
  - UNIQUE constraint on `number`
  
- `core.party_emails` - Many-to-many: parties ↔ emails
- `core.party_phones` - Many-to-many: parties ↔ phones

**Important**: When inserting emails, check for existing by `normalized_email`, not `email`

### 4. Deduplication Cache

**Current Implementation** (in-memory, single instance):
```rust
pub struct AppState {
    pub processing_leads_cache: Cache<String, i64>,  // Lead-level dedup
    pub recent_cpf_cache: Cache<String, i64>,         // CPF-level dedup
}
```

**TTL**: 5 minutes (300 seconds)  
**Capacity**: 10,000 entries

**Note**: For multi-instance deployment, migrate to Redis (see `docs/architecture/PLAN_WEBHOOK_REDIS.md`)

### 5. Rust Edition 2024

**Important**: Project uses Rust Edition 2024 (unstable)

**Dockerfile must use nightly**:
```dockerfile
FROM rust:latest as builder
RUN rustup toolchain install nightly && rustup default nightly
```

---

## Common Tasks

### Run Locally
```bash
cargo run
# or with auto-reload:
cargo watch -x run
```

### Run Tests
```bash
cargo test
```

### Build for Production
```bash
cargo build --release
```

### Deploy to Fly.io
```bash
fly deploy
# Check logs:
fly logs
# Check status:
fly status
```

### Batch Enrich CPFs
```bash
# 1. Create CPF list
echo -e "12345678901\n98765432100" > cpf_list.txt

# 2. Enrich via API (3s delay recommended)
./scripts/enrich_batch.sh https://mbras-c2s.fly.dev cpf_list.txt

# 3. Import to database
cargo run --example import_json_to_db

# Or via bash/psql:
./scripts/import_enriched_to_db.sh
```

### Database Migrations
```bash
# Connect to database
psql $DB_URL

# Run init schema
psql $DB_URL -f docs/schemas/01_init.sql
```

---

## External APIs

### Work API
- **Base URL**: `https://api.workrb.com.br/data/completa`
- **Auth**: Query param `chave=<WORK_API_KEY>`
- **Params**: `cpf=<cpf_number>` or `cep=<cep>`
- **Rate Limit**: 3 second delay recommended
- **Timeout**: Set client timeout to 60s (some queries are slow)

**Response Structure**:
```json
{
  "status": 200,
  "DadosBasicos": { "nome": "...", "cpf": "...", "sexo": "M - MASCULINO", ... },
  "DadosEconomicos": { "renda": "...", "score": {...}, ... },
  "emails": [{ "email": "...", "prioridade": "..." }],
  "telefones": [{ "telefone": "...", "tipo": "...", "whatsapp": "SIM" }],
  "enderecos": [{ "logradouro": "...", "cep": "..." }],
  "empresas": [{ "cnpj": "...", "relacao": "SOCIO" }]
}
```

### Diretrix API
- **Base URL**: `http://api.diretrixconsultoria.com.br`
- **Auth**: Basic auth (user/pass in URL or header)
- **Purpose**: Find CPF from phone/email
- **Endpoints**:
  - Search by phone: `/phone/<number>`
  - Search by email: `/email/<email>`

### Contact2Sale (C2S) API
- **Base URL**: `https://api.contact2sale.com`
- **Auth**: Bearer token in header: `Authorization: Bearer <C2S_TOKEN>`
- **Purpose**: CRM/lead management
- **Endpoints**:
  - Fetch lead: `GET /integration/lead/<lead_id>`
  - Send message: `POST /integration/messages`

---

## Deployment Configuration

### Fly.io Settings
- **App name**: `mbras-c2s`
- **Region**: `gru` (São Paulo, Brazil)
- **Memory**: 256MB
- **CPUs**: 1 shared
- **Port**: 8080
- **Auto-start**: true
- **Auto-stop**: true (when idle)
- **Min machines**: 0 (scales to zero)

**Secrets** (set via `fly secrets set`):
```bash
fly secrets set DB_URL="..."
fly secrets set WORK_API="..."
fly secrets set C2S_TOKEN="..."
fly secrets set DIRETRIX_USER="..."
fly secrets set DIRETRIX_PASS="..."
```

---

## Recent Changes & Current State

### Latest Deployment
- **Date**: 2025-01-14
- **Commit**: `42b444c` - "fix: implement atomic lead deduplication"
- **Status**: ✅ Running in production
- **URL**: https://mbras-c2s.fly.dev

### Recent Work Completed
1. ✅ Fixed duplicate message issue (lead-level deduplication)
2. ✅ Batch enriched 19 CPFs from CEP 05676-120
3. ✅ Imported all data to PostgreSQL successfully
4. ✅ Documented Work API rate limiting (3s delay)
5. ✅ Fixed email/phone association logic

### Known Issues
- ⚠️ Database has no UNIQUE constraint on `cpf_cnpj` (allows duplicate entries)
- ⚠️ In-memory cache won't work with multiple instances (need Redis for scaling)
- ⚠️ Credentials need rotation (see `docs/security/SECURITY_ROTATION_REQUIRED.md`)

---

## Future Plans

See `docs/architecture/PLAN_WEBHOOK_REDIS.md` for detailed roadmap:

1. **Direct C2S Webhooks** (eliminate Make.com dependency)
   - Create `POST /api/v1/webhook/leads` endpoint
   - Implement HMAC signature validation
   - Add `webhook_events` table for audit trail

2. **Redis Integration** (multi-instance support)
   - Replace in-memory cache with Redis
   - Use atomic `SET NX EX` for distributed locks
   - Enable horizontal scaling

3. **Better Documentation** (✅ completed)
   - ✅ Organized docs into categories (analysis/, architecture/, deployment/, integrations/, performance/, security/, sessions/)
   - ✅ Moved shell scripts from docs/ to scripts/
   - ✅ Consolidated example files into docs/examples/
   - ✅ Removed duplicate documentation files

---

## Testing

### Integration Tests
Located in `tests/` (Node.js based):
- `smoke-test.js` - Basic endpoint tests
- `load-test.js` - Performance/load testing

### Manual Testing
```bash
# Test health endpoint
curl https://mbras-c2s.fly.dev/health

# Test Work API module
curl "https://mbras-c2s.fly.dev/api/v1/work/modules/all?documento=12345678901"

# Test CEP lookup
curl "https://mbras-c2s.fly.dev/api/v1/work/modules/cep?documento=05676-120"
```

---

## Troubleshooting

### "Edition 2024 is required" error
**Solution**: Ensure Docker/local uses Rust nightly
```bash
rustup toolchain install nightly
rustup default nightly
```

### "relation core.parties does not exist"
**Solution**: Run database migrations
```bash
psql $DB_URL -f docs/schemas/01_init.sql
```

### Emails not associating with parties
**Issue**: `app.emails` has UNIQUE constraint on `normalized_email`, not `email`  
**Solution**: Query by `normalized_email = LOWER(TRIM(email))` before insert

### Work API timeouts
**Solution**: 
- Increase client timeout to 60s
- Use 3s delay between requests
- Implement retry with exponential backoff

---

## Quick Reference Commands

```bash
# Development
cargo run                          # Start server
cargo test                         # Run tests
cargo check                        # Quick compile check
cargo build --release              # Production build

# Fly.io
fly deploy                         # Deploy to production
fly logs                           # View logs
fly status                         # Check app status
fly secrets set KEY=value          # Set environment variable
fly ssh console                    # SSH into container

# Database
psql $DB_URL                       # Connect to database
psql $DB_URL -f schema.sql         # Run SQL file
psql $DB_URL -c "SELECT..."        # Run query

# Batch Processing
./scripts/enrich_batch.sh <url> <cpf_file>       # Enrich CPFs
cargo run --example import_json_to_db            # Import to DB
./scripts/retry_failed_cpfs.sh <url> <failed_file>  # Retry failures
```

---

## Contact & Support

- **Repository**: https://github.com/MbInteligen/mbras-c2s-enrichment
- **Deployment**: https://mbras-c2s.fly.dev
- **Database**: Neon.tech PostgreSQL (São Paulo region)

---

**Last Updated**: 2025-11-20  
**Maintained by**: MbInteligen Team

---

## Recent Updates (2025-11-20)

### ✅ Schema Migration & Address Confidence System

#### Database Schema Changes

The database now uses the following structure:

**Core Tables:**
- `core.entities` - People/companies (uses UUID, not separate customers/parties)
  - `entity_id` (UUID, PK)
  - `national_id` (CPF/CNPJ)
  - `name`, `canonical_name`
  - `metadata` (JSONB) - Stores `c2s_lead_id` for tracking
  - `is_enriched`, `enriched_at`

- `core.addresses` - All addresses
  - `id` (UUID, PK)
  - `street`, `number`, `neighborhood`, `city`, `state`, `zip_code`
  - `formatted_address`
  - `primary_address`, `is_valid`

- `core.entity_addresses` - N:N relationship
  - `entity_id` → `core.entities`
  - `address_id` → `core.addresses`
  - `address_type` ('residential', 'family_member', etc)
  - `is_primary`, `is_current`
  - `confidence_score` (0.0-1.0) ← **NEW!**
  - `verified` (boolean)
  - `metadata` (JSONB) - Tracks relationship, owner_name, etc

**Key Changes:**
- Fixed: `app.addresses` → `core.addresses`
- Fixed: Return type `i32` → `Uuid`
- Added: Lead tracking via `metadata->>'c2s_lead_id'`
- Added: Address confidence scoring system

#### Address Confidence Scoring System

**Problem:** Work API returns addresses that might belong to family members (spouse, parents), not the person.

**Solution:** Intelligent confidence scoring based on position and relationship detection.

**Confidence Levels:**
- 🟢 **90%** - First address, no relationship → Very likely current residence
- 🟡 **75%** - Additional addresses → May be secondary/old
- 🟠 **50%** - Spouse address → May live together
- 🔴 **40%** - Parent address → Probably doesn't live there
- 🟣 **45%** - Other family → Low probability

**Code Logic (src/db_storage.rs:454):**
```rust
let (confidence_score, address_type_str, verified) = match (idx, relationship) {
    (0, None) => (0.90, "residential", true),  // First address
    (_, Some(rel)) if rel.contains("CÔNJUGE") => (0.50, "family_member", false),
    (_, Some(rel)) if rel.contains("PAI") || rel.contains("MÃE") => (0.40, "family_member", false),
    (_, Some(_)) => (0.45, "family_member", false),
    _ => (0.75, "residential", false),
};
```

**Metadata Structure:**
```json
{
  "source": "work_api",
  "confidence_score": 0.90,
  "position_in_response": 0,
  "verified": true,
  "owner_name": "MARIA SILVA",
  "relationship": "CÔNJUGE"
}
```

#### New Database Methods

**Lead Tracking:**
```rust
// Store with lead_id tracking
storage.store_enriched_person_with_lead(cpf, work_data, Some(&lead_id)).await

// Metadata stored in entity:
{
  "c2s_lead_id": "bf1a88eaa4ab34b01a257536563fb42b",
  "c2s_source": "api_enrichment",
  "enriched_at": "2025-11-20T..."
}
```

#### Useful Queries

**Find high-confidence addresses in noble neighborhoods:**
```sql
SELECT 
    e.name,
    e.national_id,
    e.metadata->>'c2s_lead_id' as lead_id,
    a.neighborhood,
    a.city,
    ea.confidence_score,
    ea.address_type
FROM core.entities e
JOIN core.entity_addresses ea ON e.entity_id = ea.entity_id
JOIN core.addresses a ON ea.address_id = a.id
WHERE a.city ILIKE '%São Paulo%'
AND (
    a.neighborhood ILIKE '%Jardim Europa%' OR
    a.neighborhood ILIKE '%Vila Nova Conceição%' OR
    a.neighborhood ILIKE '%Cidade Jardim%' OR
    a.neighborhood ILIKE '%Itaim Bibi%' OR
    a.neighborhood ILIKE '%Moema%'
)
AND ea.confidence_score >= 0.75  -- Only medium/high confidence
ORDER BY ea.confidence_score DESC;
```

**Find entity by C2S lead_id:**
```sql
SELECT * FROM core.entities 
WHERE metadata->>'c2s_lead_id' = 'bf1a88eaa4ab34b01a257536563fb42b';
```

**View all addresses with confidence scores:**
```sql
SELECT 
    e.name,
    a.neighborhood,
    a.city,
    ea.address_type,
    ea.confidence_score,
    ea.verified,
    ea.metadata->>'relationship' as relationship
FROM core.entities e
JOIN core.entity_addresses ea ON e.entity_id = ea.entity_id
JOIN core.addresses a ON ea.address_id = a.id
WHERE e.national_id = '12345678901'
ORDER BY ea.confidence_score DESC;
```

#### Documentation Files

1. **`docs/SCHEMA_MIGRATION_LEAD_ADDRESS.md`** - Complete schema migration guide
2. **`docs/ADDRESS_CONFIDENCE_SCORING.md`** - Detailed confidence scoring system documentation

#### Key Files Modified

- `src/db_storage.rs` (lines 22-35, 154-210, 428-600)
  - Added `store_enriched_person_with_lead()` method
  - Implemented address confidence scoring
  - Fixed `app.addresses` → `core.addresses`
  - Added metadata tracking for leads and addresses

- `src/handlers.rs` (lines 440, 898)
  - Updated to use `store_enriched_person_with_lead()`
  - Pass lead_id to storage layer

#### Important Notes

1. **Backward Compatible:** Old `store_enriched_person()` still works (without lead_id)
2. **UUID vs INT:** All primary keys are UUID, not INT
3. **Metadata Merge:** Existing entity metadata is merged, not overwritten
4. **Primary Address:** First address from Work API marked as `is_primary = true`
5. **Confidence Filtering:** Always filter by `confidence_score >= 0.75` for reliable data

#### Testing

```bash
# Compile
cargo check
cargo build

# Test enrichment
curl -X POST https://mbras-c2s.fly.dev/api/v1/c2s/enrich/LEAD_ID

# Verify in database
psql $DB_URL -c "
SELECT 
    e.name,
    a.neighborhood,
    ea.confidence_score,
    ea.metadata->>'relationship'
FROM core.entities e
JOIN core.entity_addresses ea ON e.entity_id = ea.entity_id
JOIN core.addresses a ON ea.address_id = a.id
WHERE e.metadata->>'c2s_lead_id' = 'LEAD_ID'
ORDER BY ea.confidence_score DESC
"
```

#### Deployment Status

- **Compilation:** ✅ No errors (only 3 dead code warnings)
- **Testing:** ✅ Logic validated
- **Documentation:** ✅ Complete
- **Production:** ⏳ Ready for deployment

---

## C2S Gateway Integration ✅ COMPLETE

### Overview

**Status**: ✅ **Production Ready** (Commits: `56b48db`, `9d7984d`)

This Rust API now integrates with the **C2S Gateway** (Python/FastAPI) for all Contact2Sale operations, with automatic fallback to direct C2S for backward compatibility.

**Gateway Location**: `/Users/ronaldo/Documents/projects/clients/ibvi/services/ads/platform/c2s-gateway/`  
**Gateway URL**: `https://mbras-c2s-gateway.fly.dev`

### Integration Architecture (Option B - Partial)

```
Make.com Webhook
    ↓
Rust C2S API (mbras-c2s.fly.dev)
    │
    ├→ if C2S_GATEWAY_URL set:
    │   └→ Python C2S Gateway → Contact2Sale API
    │
    ├→ else (fallback):
    │   └→ Contact2Sale API (direct)
    │
    ├→ Work API (enrichment data)
    ├→ Diretrix API (CPF lookup)
    └→ PostgreSQL (storage)
```

### Current Implementation

**Gateway Client**: `src/gateway_client.rs`
```rust
#[derive(Clone)]
pub struct C2sGatewayClient {
    // Methods: get_lead, send_message, update_lead, list_leads
}
```

**Migrated Endpoints**:
- ✅ `/api/v1/c2s/enrich/:lead_id` - Uses gateway when available
- ✅ `/api/v1/leads/process` - Uses gateway when available

**Initialization** (`src/main.rs`):
- Automatically initializes gateway if `C2S_GATEWAY_URL` is set
- Falls back to direct C2S if not configured
- Logs which path is being used

### Configuration

**To Enable Gateway** (Optional):
```bash
# Local
echo "C2S_GATEWAY_URL=https://mbras-c2s-gateway.fly.dev" >> .env

# Production (Fly.io)
fly secrets set C2S_GATEWAY_URL=https://mbras-c2s-gateway.fly.dev
```

**Without Config**: Automatically uses direct C2S (backward compatible)

### Benefits Now Available

When `C2S_GATEWAY_URL` is set:

1. **Campaign enrichment** (Google Ads → property mapping)
2. **28+ C2S endpoints** accessible via gateway
3. **Better error handling** with Pydantic validation
4. **Centralized token management**
5. **~45-50ms gateway response time**

### Testing

**Integration Tests**: `./scripts/test_gateway_integration.sh`
```
Test Results: 8/8 PASS
Gateway Response Time: 49ms
Integration Status: READY
```

### Logs Example

```
INFO rust_c2s_api: ✓ C2S Gateway client initialized: https://mbras-c2s-gateway.fly.dev
INFO rust_c2s_api::handlers: Using C2S Gateway to fetch lead
INFO rust_c2s_api::handlers: Using C2S Gateway to send message
```

### Documentation

- **Complete Guide**: `docs/architecture/GATEWAY_INTEGRATION_COMPLETE.md`
- **Decision Guide**: `docs/architecture/INTEGRATION_DECISION.md`
- **Original Plan**: `docs/architecture/C2S_GATEWAY_INTEGRATION.md`

### Rollback

To disable gateway integration:
```bash
fly secrets unset C2S_GATEWAY_URL
# System automatically falls back to direct C2S
```

**Note**: Integration is backward compatible. Works with or without gateway URL configured.

---

