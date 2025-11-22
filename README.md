# rust-c2s-api

Rust-based API for Contact2Sale (C2S) lead enrichment using Diretrix and Work API integrations.

## Features

- 🚀 **Lead Processing**: Automated enrichment pipeline for C2S leads
- 📞 **Multi-source Lookup**: Phone + Email → CPF resolution via Diretrix
- 💼 **Complete Enrichment**: Personal, financial, and contact data via Work API
- 💾 **Database Storage**: Persistent storage in PostgreSQL (Neon)
- 🔄 **Make.com Integration**: Simple trigger endpoint for automation
- ⚡ **High Performance**: Built with Axum and async Rust
- 🎯 **Smart Deduplication**: In-memory cache prevents redundant API calls (67% cost savings)

## Architecture

```
Make.com → rust-c2s-api → C2S API (fetch lead)
                        ↓
                   Diretrix API (get CPF)
                        ↓
                    Work API (enrich)
                        ↓
                   PostgreSQL (store)
                        ↓
                   C2S API (send message)
```

## Quick Start

### Prerequisites

- Rust 1.75+ (`rustup install stable`)
- PostgreSQL 15+ (or Neon account)
- Docker (for testing)
- k6 (for load testing): `brew install k6`

### Local Development

```bash
# 1. Clone and setup
git clone <repo>
cd rust-c2s-api
cp .env.example .env
# Edit .env with your credentials

# 2. Run migrations (if using local Postgres)
sqlx migrate run

# 3. Build and run
cargo run

# 4. Test
./docs/scripts/testing/test-local.sh
```

### Docker Testing

```bash
# Full stack test with isolated database
./docs/scripts/testing/test-docker.sh
```

### Deploy to Fly.io

```bash
# First time setup
fly launch

# Subsequent deploys
fly deploy

# Check status
fly status --app rust-c2s-api
fly logs -f
```

## API Endpoints

### Main Endpoint (Make.com)

```http
GET /api/v1/leads/process?id={lead_id}
```

**Purpose**: Trigger lead enrichment from Make.com

**Flow**:
1. Fetch lead from C2S
2. Find CPF via Diretrix (phone + email)
3. Enrich with Work API
4. Store in database
5. Send enriched message to C2S

**Example**:
```bash
curl "https://your-app.fly.dev/api/v1/leads/process?id=358f62821dc6cfa7cfbda19e670d6392"
```

**Response**:
```json
{
  "success": true,
  "message": "Successfully processed and enriched lead. Stored 1 entities in database.",
  "lead_id": "358f62821dc6cfa7cfbda19e670d6392",
  "cpfs_processed": ["12345678900"],
  "entities_stored": 1
}
```

### Other Endpoints

- `GET /health` - Health check
- `GET /api/v1/contributor/customer?cpf={cpf}` - Get customer by CPF
- `GET /api/v1/contributor/customer?email={email}` - Get customer by email
- `GET /api/v1/contributor/customer?phone={phone}` - Get customer by phone
- `GET /api/v1/contributor/customer?name={name}` - Get customer by name
- `POST /api/v1/enrich` - Enrich customer (JSON body)
- `GET /api/v1/work/modules/all?documento={cpf}` - Work API full data
- `POST /api/v1/c2s/enrich/:lead_id` - Direct C2S enrichment

## Environment Variables

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

# Logging (optional)
RUST_LOG=info  # or debug for verbose
```

## Testing

### Quick Tests
```bash
# Unit tests
cargo test

# Integration tests
./docs/scripts/testing/test-local.sh

# Docker integration
./docs/scripts/testing/test-docker.sh

# Smoke test
k6 run tests/smoke-test.js

# Load test
k6 run tests/load-test.js
```

### Documentation
- [Documentation Index](docs/README.md) - Complete documentation navigation
- [Quick Start Guide](docs/QUICKSTART.md)
- [API Endpoints](docs/API_ENDPOINTS.md)
- [Database Schema Report](docs/database/DATABASE_SCHEMA_REPORT_FINAL.md)
- [Architecture Decision Records](docs/adr/)
- [Testing Guide](docs/testing/TESTING.md)
- [Make.com Integration](docs/integrations/MAKE_INTEGRATION.md)
- [Security Checklist](docs/security/SECURITY_CHECKLIST.md)

## Project Structure

```
rust-c2s-api/
├── src/                  # Source code
│   ├── main.rs          # Application entry point & routing
│   ├── config.rs        # Configuration management
│   ├── db.rs            # Database connection
│   ├── db_storage.rs    # Enrichment data storage
│   ├── errors.rs        # Error types & handling
│   ├── handlers.rs      # HTTP request handlers
│   ├── models.rs        # Data models
│   └── services.rs      # External API integrations
│
├── docs/                 # All documentation and resources
│   ├── adr/             # Architecture Decision Records
│   ├── architecture/    # System design documents
│   ├── database/        # Database docs + examples
│   │   └── examples/    # JSON responses + Rust examples
│   ├── deployment/      # Deployment guides
│   ├── integrations/    # External API documentation
│   ├── queries/         # SQL query examples
│   ├── schemas/         # Database schema files
│   ├── scripts/         # Utility scripts
│   │   ├── data/       # Data processing
│   │   ├── deployment/ # Deployment scripts
│   │   └── testing/    # Test scripts
│   ├── security/        # Security documentation
│   ├── session-notes/   # Development summaries
│   └── testing/         # Test documentation
│
├── tests/               # k6 load/smoke tests
├── target/              # Rust build artifacts
├── Dockerfile           # Container image
├── fly.toml             # Fly.io configuration
└── docker-compose*.yml  # Docker environments
```

## Database Schema

**PostgreSQL 17.5** (Neon.tech) with **Party Model** architecture:

**Core Tables**:
- `core.parties` - Golden record (1.5M+ records)
- `core.people` - Person-specific attributes (1.1M+)
- `core.companies` - Company-specific attributes (412K+)
- `core.party_contacts` - Unified contacts (email/phone/whatsapp)
- `core.party_enrichments` - Enrichment tracking
- `core.real_estate_properties` - Property ownership

**Analytics Layer**:
- `core.mv_party_analytics` - Base analytics materialized view
- `analytics.mv_mkt_lead_star` - Marketing star schema

See [DATABASE_SCHEMA_REPORT_FINAL.md](docs/database/DATABASE_SCHEMA_REPORT_FINAL.md) for complete details.

## Performance

**Resource Usage** (1 GB RAM, Shared CPU):
- Idle: 80-150 MB memory
- Load: 200-400 MB memory
- Peak: <700 MB memory

**Latency**:
- Health check: <50ms (p95)
- Database queries: <200ms (p95)
- Full enrichment: <5s (p95)

**Throughput**:
- Simple queries: 50+ req/s
- Full enrichment: 2-5 req/s (limited by external APIs)

See [PERFORMANCE_MONITORING.md](docs/testing/PERFORMANCE_MONITORING.md) for optimization.

## Security

All security-sensitive configurations have been addressed:

- ✅ No hardcoded credentials
- ✅ Mandatory environment variables
- ✅ `.env.example` template provided
- ✅ Proper error handling
- ✅ Database queries use production schema

See [SECURITY_AND_SCHEMA_FIXES.md](docs/SECURITY_AND_SCHEMA_FIXES.md).

## Make.com Integration

### Current Setup

Replace Cloud Function with direct Rust service call:

**Old**:
```
C2S → Make → Cloud Function → ...
```

**New**:
```
C2S → Make → rust-c2s-api
```

**Configuration**:
```
URL: https://your-app.fly.dev/api/v1/leads/process?id={{lead.id}}
Method: GET
```

See [MAKE_INTEGRATION.md](docs/MAKE_INTEGRATION.md) for complete setup.

## Deployment

### Fly.io

```bash
# Deploy
fly deploy

# View logs
fly logs -f

# Check status
fly status --app rust-c2s-api

# Scale
fly scale memory 512  # Reduce to 512MB
fly scale count 2     # Add instance for HA
```

### Resource Sizing

**Current**: 1 GB RAM, Shared CPU

**Options**:
- **512 MB**: For low traffic (<50 req/min)
- **1 GB**: Safe default for moderate traffic
- **2+ instances**: For high availability

See [PERFORMANCE_MONITORING.md](docs/PERFORMANCE_MONITORING.md#vm-sizing-strategy).

## Monitoring

```bash
# Real-time metrics
fly status --app rust-c2s-api

# Live logs
fly logs -f --app rust-c2s-api

# Filter errors
fly logs | grep ERROR

# Check memory
fly ssh console -C "free -h"
```

## Troubleshooting

### Common Issues

**"Connection refused"**
```bash
# Check if server is running
fly status --app rust-c2s-api

# Restart
fly deploy --force
```

**"Failed to fetch lead from C2S"**
- Verify C2S_TOKEN is correct
- Check C2S_BASE_URL
- Confirm lead ID exists

**"Could not find CPF"**
- Lead has invalid phone/email
- Diretrix API may be down
- Check Diretrix credentials

**"Out of memory"**
```bash
# Check usage
fly status

# Increase memory
fly scale memory 1024
```

See [TESTING.md](docs/TESTING.md#troubleshooting-tests) for more.

## Contributing

1. Create feature branch
2. Make changes
3. Run tests: `cargo test && ./docs/scripts/test-local.sh`
4. Format code: `cargo fmt`
5. Check lints: `cargo clippy`
6. Submit PR

## License

[Add your license]

## Support

- [Documentation](docs/)
- [GitHub Issues](https://github.com/your-org/rust-c2s-api/issues)

---

**Built with** 🦀 Rust • ⚡ Axum • 🐘 PostgreSQL • 🚀 Fly.io
