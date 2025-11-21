# Testing Guide for rust-c2s-api

Complete testing documentation for local development, CI/CD, and production validation.

---

## Quick Start

```bash
# 1. Local testing (fastest)
cargo test                    # Unit tests
./test-local.sh              # Integration tests

# 2. Docker testing (production-like)
./test-docker.sh             # Full stack test

# 3. Load testing (before deployment)
k6 run tests/smoke-test.js   # Quick validation
k6 run tests/load-test.js    # Full load test

# 4. Production testing (after deployment)
fly deploy
./test-local.sh https://your-app.fly.dev
```

---

## Test Suite Overview

### 1. Unit Tests (Rust)

**Location**: `src/**/tests/` or inline `#[cfg(test)]` modules

**Run:**
```bash
# All tests
cargo test

# Specific test
cargo test test_name

# With output
cargo test -- --nocapture

# Integration tests only
cargo test --test '*'
```

**Coverage:**
- Data models serialization/deserialization
- Error handling
- Helper functions
- Business logic

**TODO**: Add more unit tests as code evolves

---

### 2. Integration Tests (Bash + curl)

**Script**: `test-local.sh`

**Tests:**
1. ✅ Health check endpoint
2. ✅ Trigger lead processing (main Make.com flow)
3. ✅ Get customer by CPF
4. ✅ Get customer by email
5. ✅ Get customer by phone
6. ✅ Get customer by name
7. ✅ Enrich customer (POST JSON)
8. ✅ Work API module fetching

**Usage:**
```bash
# Local server
./test-local.sh

# Remote server
./test-local.sh https://your-app.fly.dev

# Custom lead ID
./test-local.sh https://your-app.fly.dev YOUR_LEAD_ID
```

**Expected output:**
```
🧪 Testing rust-c2s-api at http://localhost:8081
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Testing: Health Check
  → GET /health
  ✓ HTTP 200
{
  "status": "ok",
  "service": "rust-c2s-api"
}

Testing: Trigger Lead Processing
  → GET /api/v1/leads/process?id=358f62...
  ✓ HTTP 200
{
  "success": true,
  "message": "Successfully processed...",
  "lead_id": "358f62...",
  "cpfs_processed": ["12345678900"],
  "entities_stored": 1
}
...
```

---

### 3. Docker Integration Tests

**Script**: `test-docker.sh`
**Compose**: `docker-compose.test.yml`

**Components:**
- PostgreSQL 15 test database
- Rust application container
- Test isolation from production

**Usage:**
```bash
# Automated test run
./test-docker.sh

# Manual testing
docker-compose -f docker-compose.test.yml up -d
./test-local.sh http://localhost:8081
docker-compose -f docker-compose.test.yml down
```

**Benefits:**
- Tests production Docker image
- Isolated test database
- Validates database migrations
- Catches container-specific issues

---

### 4. Smoke Tests (k6)

**Script**: `tests/smoke-test.js`

**Purpose**: Quick validation with minimal load

**Profile:**
- 1 virtual user
- 30 second duration
- Basic endpoints only

**Usage:**
```bash
# Local
k6 run tests/smoke-test.js

# Production
k6 run -e BASE_URL=https://your-app.fly.dev tests/smoke-test.js
```

**Success criteria:**
- ✅ p95 latency <3s
- ✅ Error rate <5%
- ✅ All health checks pass

**When to use:**
- ✅ After every deployment
- ✅ Before load testing
- ✅ Quick sanity check

---

### 5. Load Tests (k6)

**Script**: `tests/load-test.js`

**Purpose**: Capacity planning and performance validation

**Profile:**
- Ramp: 0→5→10→20→0 users
- Duration: 3.5 minutes total
- Mixed endpoint testing
- External API throttling (1/10 full flows)

**Metrics tracked:**
- Total requests
- Request duration (avg, median, p95, max)
- Error rate
- Custom error tracking

**Usage:**
```bash
# Default (localhost)
k6 run tests/load-test.js

# Custom target
k6 run -e BASE_URL=https://your-app.fly.dev tests/load-test.js

# Custom lead ID
k6 run \
  -e BASE_URL=https://your-app.fly.dev \
  -e LEAD_ID=your_lead_id \
  tests/load-test.js
```

**Expected output:**
```
📊 Load Test Summary
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Total Requests: 1247
Request Duration:
  Average: 156.32ms
  Median:  98.45ms
  P95:     487.23ms
  Max:     1543.12ms
Failed Requests: 2.1%
Error Rate: 1.8%

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
💡 Next steps:
  - Check fly logs for errors
  - Monitor memory: fly status --app rust-c2s-api
  - Adjust VM size if needed in fly.toml
```

**When to use:**
- ✅ Before first deployment
- ✅ After major changes
- ✅ Monthly capacity validation
- ✅ Before VM size changes

---

## Testing Workflow

### Pre-Deployment Checklist

```bash
# 1. Code quality
cargo fmt --check
cargo clippy
cargo test

# 2. Local integration
./test-local.sh

# 3. Docker validation
./test-docker.sh

# 4. Smoke test
k6 run tests/smoke-test.js

# 5. If all pass → deploy
fly deploy

# 6. Post-deployment validation
k6 run -e BASE_URL=https://your-app.fly.dev tests/smoke-test.js
fly logs -f
```

### Monthly Performance Check

```bash
# 1. Full load test
k6 run -e BASE_URL=https://your-app.fly.dev tests/load-test.js

# 2. Review metrics
fly status --app rust-c2s-api
fly logs | grep -i "error\|warning"

# 3. Check database
# Connect to Neon dashboard
# Review query performance, connection pool usage

# 4. Optimize if needed
# - Reduce memory if usage <50%
# - Add instances if p95 >2s
# - Upgrade CPU if throttling
```

---

## Test Data Setup

### Database Seed Data

**For local testing:**
```sql
-- Connect to test database
psql postgresql://test_user:test_password@localhost:5433/rust_c2s_test

-- Insert test entity
INSERT INTO core.entities (national_id, name, entity_type)
VALUES ('12345678900', 'João Silva', 'person'::core.entity_type_enum);

-- Insert test contacts
INSERT INTO core.entity_emails (entity_id, email)
SELECT entity_id, 'test@example.com'
FROM core.entities WHERE national_id = '12345678900';

INSERT INTO core.entity_phones (entity_id, phone)
SELECT entity_id, '5511999998888'
FROM core.entities WHERE national_id = '12345678900';
```

### Mock External APIs

**Option 1: Use real APIs** (requires valid credentials)
- Set up `.env` with production tokens
- Limited by rate limits
- Tests actual integration

**Option 2: Use mocks** (for CI/CD)
```bash
# TODO: Add mockserver configuration
docker run -d -p 1080:1080 mockserver/mockserver
# Configure mock responses in tests/mockserver-init.json
```

---

## Environment-Specific Testing

### Local Development
```bash
# .env.local
DB_URL=postgresql://localhost/rust_c2s_dev
C2S_BASE_URL=https://api-staging.contact2sale.com  # Staging
RUST_LOG=debug
```

### Docker Testing
```bash
# Uses docker-compose.test.yml
# Isolated test database
# Production-like container
```

### Staging (Fly.io)
```bash
# Deploy to staging app
fly deploy --app rust-c2s-api-staging

# Test against staging
./test-local.sh https://rust-c2s-api-staging.fly.dev
```

### Production (Fly.io)
```bash
# Smoke tests only (no load tests on prod!)
k6 run -e BASE_URL=https://your-app.fly.dev tests/smoke-test.js

# Monitor real traffic
fly logs -f --app rust-c2s-api
```

---

## CI/CD Integration

### GitHub Actions Example

```yaml
name: Test

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    
    services:
      postgres:
        image: postgres:15
        env:
          POSTGRES_DB: test
          POSTGRES_USER: test
          POSTGRES_PASSWORD: test
        options: >-
          --health-cmd pg_isready
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
    
    steps:
      - uses: actions/checkout@v3
      
      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      
      - name: Run tests
        env:
          DATABASE_URL: postgresql://test:test@localhost/test
        run: |
          cargo test
          cargo clippy -- -D warnings
          cargo fmt --check
      
      - name: Build Docker image
        run: docker build -t rust-c2s-api .
```

---

## Troubleshooting Tests

### "Connection refused"
```bash
# Ensure server is running
cargo run &
# or
docker-compose -f docker-compose.test.yml up -d

# Check port
netstat -an | grep 8081
```

### "Database connection failed"
```bash
# Check DATABASE_URL
echo $DB_URL

# Verify PostgreSQL is running
docker ps | grep postgres
pg_isready -h localhost -p 5433

# Run migrations
sqlx migrate run
```

### "External API errors"
```bash
# Verify credentials
echo $C2S_TOKEN
echo $WORK_API
echo $DIRETRIX_USER

# Test APIs directly
curl -H "Authorization: Bearer $C2S_TOKEN" \
  https://api.contact2sale.com/integration/leads/test

# Check rate limits
fly logs | grep "rate limit"
```

### "k6 not found"
```bash
# Install k6
# macOS
brew install k6

# Linux
sudo gpg -k
sudo gpg --no-default-keyring --keyring /usr/share/keyrings/k6-archive-keyring.gpg --keyserver hkp://keyserver.ubuntu.com:80 --recv-keys C5AD17C747E3415A3642D57D77C6C491D6AC1D69
echo "deb [signed-by=/usr/share/keyrings/k6-archive-keyring.gpg] https://dl.k6.io/deb stable main" | sudo tee /etc/apt/sources.list.d/k6.list
sudo apt-get update
sudo apt-get install k6

# Windows
choco install k6
```

---

## Performance Baselines

### Expected Metrics (1 GB RAM, Shared CPU)

**Latency:**
| Endpoint | p50 | p95 | p99 |
|----------|-----|-----|-----|
| /health | 5ms | 15ms | 30ms |
| /api/v1/contributor/customer | 50ms | 200ms | 500ms |
| /api/v1/leads/process | 2s | 5s | 8s |

**Throughput:**
| Type | Req/s |
|------|-------|
| Simple queries | 50+ |
| Full enrichment | 2-5 |

**Memory:**
- Idle: 80-150 MB
- Load: 200-400 MB
- Peak: <700 MB

**If metrics deviate significantly, investigate!**

---

## Next Steps

- [ ] Add Rust unit tests for handlers
- [ ] Set up GitHub Actions CI/CD
- [ ] Create staging environment on Fly.io
- [ ] Add database migration tests
- [ ] Implement API mocks for offline testing
- [ ] Add Prometheus metrics
- [ ] Set up Grafana dashboards
- [ ] Configure alerting (PagerDuty, Slack)

---

Generated: 2025-01-13
