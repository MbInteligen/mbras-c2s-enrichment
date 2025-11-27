# Quick Start Guide

Get rust-c2s-api running in 5 minutes.

---

## 1. Setup Environment (2 min)

```bash
# Clone repo
cd rust-c2s-api

# Create .env from template
cp .env.example .env

# Edit .env with your credentials
# Required: C2S_TOKEN, WORK_API, DIRETRIX_USER, DIRETRIX_PASS, DB_URL
nano .env
```

---

## 2. Test Locally (1 min)

```bash
# Build and run
cargo run

# In another terminal, test
./test-local.sh
```

**Expected**: All tests pass ✅

---

## 3. Deploy to Fly.io (2 min)

```bash
# Set secrets
fly secrets set \
  C2S_TOKEN="your_token" \
  WORK_API="your_key" \
  DIRETRIX_USER="your_user" \
  DIRETRIX_PASS="your_pass" \
  DB_URL="postgresql://..." \
  --app rust-c2s-api

# Deploy
fly deploy

# Verify
fly status --app rust-c2s-api
```

---

## 4. Update Make.com

**Old URL:**
```
https://us-central1-xxx.cloudfunctions.net/processLead?id={{lead_id}}
```

**New URL:**
```
https://your-app.fly.dev/api/v1/leads/process?id={{lead_id}}
```

**Save** and test in Make.com

---

## 5. Monitor

```bash
# Watch logs
fly logs -f

# Check status
fly status

# Test endpoint
curl "https://your-app.fly.dev/health"
```

---

## Common Commands

```bash
# Local
cargo run                    # Start server
cargo test                   # Run tests
./test-local.sh             # Integration tests

# Docker
./test-docker.sh            # Full stack test

# Deployment  
fly deploy                  # Deploy
fly logs -f                 # Monitor
fly status                  # Check resources

# Testing
k6 run tests/smoke-test.js  # Quick test
k6 run tests/load-test.js   # Load test
```

---

## Endpoints

```
GET  /health                              # Health check
GET  /api/v1/leads/process?id={id}       # Main endpoint (Make.com)
GET  /api/v1/contributor/customer?cpf={} # Get customer
POST /api/v1/enrich                       # Enrich (JSON body)
```

---

## Troubleshooting

**Build fails?**
```bash
cargo clean
cargo build
```

**Tests fail?**
```bash
# Check .env
cat .env

# Verify DB connection
psql "$DB_URL"
```

**Deployment fails?**
```bash
# Check logs
fly logs

# Verify secrets
fly secrets list
```

---

## Documentation

- **Full docs**: [README.md](README.md)
- **Testing**: [docs/TESTING.md](docs/TESTING.md)
- **Deployment**: [docs/DEPLOYMENT_CHECKLIST.md](docs/DEPLOYMENT_CHECKLIST.md)
- **Monitoring**: [docs/PERFORMANCE_MONITORING.md](docs/PERFORMANCE_MONITORING.md)

---

## Support

- Logs: `fly logs -f`
- Status: `fly status`
- Docs: `./docs/`

---

**You're ready to go! 🚀**
