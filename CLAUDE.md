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
├── scripts/                  # Bash scripts
│   ├── enrich_batch.sh      # Batch enrichment via API endpoint
│   ├── retry_failed_cpfs.sh # Retry failed enrichments
│   └── import_enriched_to_db.sh  # Import via psql
│
├── docs/                     # Documentation
│   ├── api/                 # API documentation
│   ├── architecture/        # (planned) Architecture docs
│   ├── deployment/          # (planned) Deployment guides
│   ├── operations/          # (planned) Operations & maintenance
│   ├── sql/                 # (planned) SQL reference queries
│   ├── schemas/             # Database schemas
│   ├── scripts/             # (legacy) Old test scripts
│   ├── queries/             # SQL query examples
│   ├── WORK_API_RATE_LIMITING.md  # Rate limiting guidelines
│   └── PLAN_WEBHOOK_REDIS.md      # Future: Direct C2S webhooks + Redis
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
- See `docs/WORK_API_RATE_LIMITING.md` for details
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

**Note**: For multi-instance deployment, migrate to Redis (see `docs/PLAN_WEBHOOK_REDIS.md`)

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
- ⚠️ Credentials need rotation (see `docs/SECURITY_ROTATION_REQUIRED.md`)

---

## Future Plans

See `docs/PLAN_WEBHOOK_REDIS.md` for detailed roadmap:

1. **Direct C2S Webhooks** (eliminate Make.com dependency)
   - Create `POST /api/v1/webhook/leads` endpoint
   - Implement HMAC signature validation
   - Add `webhook_events` table for audit trail

2. **Redis Integration** (multi-instance support)
   - Replace in-memory cache with Redis
   - Use atomic `SET NX EX` for distributed locks
   - Enable horizontal scaling

3. **Better Documentation** (ongoing)
   - Organize docs into categories (api/, architecture/, deployment/, operations/)
   - Consolidate deployment guides
   - Add more code examples

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

**Last Updated**: 2025-01-14  
**Maintained by**: MbInteligen Team
