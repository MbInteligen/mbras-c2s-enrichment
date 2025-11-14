# Performance Monitoring & VM Sizing Guide

## Overview
This guide covers performance testing, monitoring, and resource optimization for rust-c2s-api on Fly.io.

---

## Current Configuration

**Fly.io VM (fly.toml):**
- **CPU**: 1 shared vCPU
- **Memory**: 1024 MB (1 GB)
- **Instances**: 1 (can scale to 2+ for HA)

**Expected Resource Usage:**
- **Rust (Axum + SQLx)**: Typically 50-200 MB at idle
- **Connection pools**: ~20-50 MB per pool
- **Request handling**: +10-50 MB per concurrent request
- **Estimated peak**: 300-500 MB under normal load

---

## Testing Strategy

### 1. Local Testing (Development)

**Quick validation:**
```bash
# Start local server
cargo run

# Run test suite
./test-local.sh http://localhost:8081

# Test specific endpoint
curl "http://localhost:8081/health"
curl "http://localhost:8081/api/v1/leads/process?id=YOUR_LEAD_ID"
```

**With Docker (production-like):**
```bash
# Build and test in container
./test-docker.sh

# Or manually:
docker build -t rust-c2s-api .
docker run --env-file .env -p 8081:8081 rust-c2s-api
```

### 2. Smoke Testing (Pre-deployment)

Quick validation with minimal load:

```bash
# Install k6 (macOS)
brew install k6

# Run smoke test
k6 run tests/smoke-test.js

# Against deployed app
k6 run -e BASE_URL=https://your-app.fly.dev tests/smoke-test.js
```

**Expected results:**
- ✅ All requests complete in <3s (p95)
- ✅ Error rate <5%
- ✅ Health check always succeeds

### 3. Load Testing (Capacity Planning)

Simulate realistic traffic:

```bash
# Run load test locally
k6 run tests/load-test.js

# Against Fly.io deployment
k6 run -e BASE_URL=https://your-app.fly.dev tests/load-test.js

# Custom lead ID
k6 run -e BASE_URL=https://your-app.fly.dev \
       -e LEAD_ID=YOUR_LEAD_ID \
       tests/load-test.js
```

**Load test profile:**
- Ramp up: 0 → 5 users (30s)
- Steady: 10 users (1 min)
- Peak: 20 users (1.5 min)
- Ramp down: 20 → 0 (30s)

**What to monitor:**
- Request duration (p95 should stay <2s)
- Error rate (should stay <10%)
- Memory usage (check `fly status`)
- CPU utilization (watch for throttling)

---

## Monitoring on Fly.io

### Real-time Monitoring

**Check application status:**
```bash
# View current resource usage
fly status --app rust-c2s-api

# Example output:
# ID       = 12345678
# Status   = running
# Memory   = 234 MB / 1024 MB (23%)
# CPU      = 5%
```

**View live logs:**
```bash
# All logs
fly logs --app rust-c2s-api

# Follow mode
fly logs -f --app rust-c2s-api

# Filter by level
fly logs --app rust-c2s-api | grep ERROR
fly logs --app rust-c2s-api | grep "Step 1:"
```

**SSH into running instance:**
```bash
# Access running container
fly ssh console --app rust-c2s-api

# Check memory inside container
free -h
ps aux --sort=-%mem | head
```

### Metrics to Track

**1. Memory (RSS)**
```bash
# Via fly status
fly status --app rust-c2s-api

# Inside container
fly ssh console --app rust-c2s-api -C "free -h"
```

**What to look for:**
- Idle: 50-150 MB
- Normal load: 200-400 MB
- Peak load: 400-600 MB
- ⚠️ Alert if >800 MB sustained
- 🚨 OOM if >1024 MB

**2. CPU Utilization**
- Shared CPU should stay <50% average
- Brief spikes to 100% are OK
- Sustained 100% = need dedicated CPU

**3. Request Metrics (from k6)**
- **Throughput**: Requests/second
- **Latency**: p50, p95, p99
- **Error rate**: Failed requests %

**4. Database Connections**
Check pool usage in logs:
```bash
fly logs | grep "connection pool"
```

---

## VM Sizing Strategy

### Current: 1 GB RAM, Shared CPU

**When it's sufficient:**
- ✅ <100 requests/minute
- ✅ Low concurrency (<10 simultaneous requests)
- ✅ Memory stays <700 MB
- ✅ No OOM kills in logs

**When to scale up:**
- ❌ Memory consistently >800 MB
- ❌ OOM kills in logs
- ❌ CPU at 100% for extended periods
- ❌ p95 latency >3s

### Scaling Options

#### Option 1: Reduce to 512 MB (Cost Optimization)

**Test first:**
```bash
# Update fly.toml
# Change: memory = '512mb'

# Deploy and monitor
fly deploy
fly logs -f

# Run load test
k6 run -e BASE_URL=https://your-app.fly.dev tests/load-test.js

# Watch for OOM
fly logs | grep -i "out of memory"
```

**Safe if:**
- Peak memory <400 MB during load tests
- No OOM kills after 24h of production traffic
- p95 latency unchanged

#### Option 2: Scale Horizontally (High Availability)

```bash
# Add second instance
fly scale count 2 --app rust-c2s-api

# Fly.io auto load-balances between instances
```

**Benefits:**
- Zero-downtime deploys
- Automatic failover
- 2x throughput capacity

**Costs:**
- 2x the resources

#### Option 3: Upgrade CPU (Performance)

```bash
# Edit fly.toml
# Change to dedicated CPU:
[vm]
  cpu_kind = "performance"
  cpus = 1
  memory_mb = 1024

# Deploy
fly deploy
```

**Use when:**
- CPU consistently >70%
- Request processing is CPU-bound
- Need faster response times

---

## Optimization Checklist

### Before Reducing Memory

- [ ] Run load test: `k6 run tests/load-test.js`
- [ ] Measure peak memory during test
- [ ] Add 30% buffer for safety margin
- [ ] Deploy with new memory limit
- [ ] Monitor for 24 hours
- [ ] Check for OOM kills: `fly logs | grep OOM`
- [ ] Verify latency unchanged

### Database Connection Pool Tuning

Current defaults (likely in code):
```rust
// Check src/db.rs or main.rs
max_connections: 10  // Adjust based on load
```

**Recommendations:**
- **Low traffic** (<10 req/min): 5 connections
- **Medium traffic** (10-50 req/min): 10 connections
- **High traffic** (>50 req/min): 20 connections

Each connection uses ~5-10 MB.

### Application-Level Optimizations

**1. Reduce log verbosity in production:**
```bash
# In .env or fly.toml
RUST_LOG=info  # Instead of debug
```

**2. Enable HTTP compression** (if not already):
```rust
// In main.rs
.layer(CompressionLayer::new())
```

**3. Add request timeouts:**
```rust
// Prevent long-running requests from accumulating
.layer(TimeoutLayer::new(Duration::from_secs(30)))
```

---

## Alerting & Monitoring Setup

### Fly.io Metrics (via fly.io dashboard)

1. Go to https://fly.io/apps/rust-c2s-api
2. View Metrics tab
3. Monitor:
   - Memory usage over time
   - CPU utilization
   - Request rate
   - Error rate

### Custom Metrics (Optional)

**Add Prometheus metrics:**
```rust
// Cargo.toml
metrics-exporter-prometheus = "0.12"

// main.rs
let recorder = PrometheusBuilder::new().build();
metrics::set_recorder(&recorder);

// Expose /metrics endpoint
.route("/metrics", get(metrics_handler))
```

**Then use Grafana Cloud or similar to visualize.**

---

## Testing Workflow

### Pre-Deployment Testing

```bash
# 1. Build and test locally
cargo build --release
cargo test

# 2. Test in Docker
./test-docker.sh

# 3. Run smoke test
k6 run tests/smoke-test.js

# 4. Deploy to Fly.io
fly deploy

# 5. Smoke test deployed app
k6 run -e BASE_URL=https://your-app.fly.dev tests/smoke-test.js

# 6. Check logs
fly logs -f
```

### Load Testing Schedule

**Initial deployment:**
- Run full load test
- Monitor for 1 hour
- Adjust resources if needed

**After each deployment:**
- Run smoke test
- Monitor logs for errors
- Check metrics dashboard

**Monthly:**
- Run full load test
- Review capacity trends
- Plan for growth

---

## Performance Benchmarks

### Target Metrics

**Latency (p95):**
- Health check: <50ms
- Database queries: <100ms
- Full enrichment flow: <5s (includes 3 external APIs)

**Throughput:**
- Minimum: 10 req/s (simple endpoints)
- Enrichment: 2-5 req/s (due to external API limits)

**Memory:**
- Idle: <150 MB
- Under load: <500 MB
- Peak: <700 MB

**Error Rate:**
- Target: <1%
- Alert: >5%
- Critical: >10%

---

## Troubleshooting

### High Memory Usage

**Diagnose:**
```bash
# Check current usage
fly status

# View memory over time
fly ssh console -C "free -h && ps aux --sort=-%mem"
```

**Common causes:**
- Too many database connections
- Memory leak (check long-running processes)
- Large response payloads being buffered

**Solutions:**
- Reduce connection pool size
- Add pagination for large result sets
- Enable streaming responses
- Upgrade memory

### High CPU Usage

**Diagnose:**
```bash
fly ssh console -C "top -b -n 1"
```

**Common causes:**
- CPU-intensive operations (JSON parsing, regex)
- High request volume
- Inefficient queries

**Solutions:**
- Optimize hot code paths
- Add caching
- Upgrade to dedicated CPU
- Scale horizontally

### OOM Kills

**Check logs:**
```bash
fly logs | grep -i "out of memory\|oom\|killed"
```

**Solutions:**
- Increase memory limit
- Reduce connection pool
- Add request size limits
- Profile memory usage

### Slow Requests

**Check logs for timing:**
```bash
fly logs | grep "Step [0-9]:"
```

**Bottlenecks:**
- Step 1 (C2S fetch): C2S API slow → check C2S status
- Step 2 (Diretrix): Diretrix API slow → contact support
- Step 3 (Work API): Work API slow → check rate limits
- Step 5 (Database): DB slow → check Neon metrics

---

## Cost Optimization

### Current Estimate (Fly.io)

**Single instance (1 GB RAM):**
- Shared CPU: ~$2.50/month
- Bandwidth: ~$0.02/GB
- **Total**: ~$3-5/month for low traffic

**Optimization paths:**
1. **512 MB RAM**: ~$1.50/month (save 40%)
2. **Keep 1 GB**: Better headroom for spikes
3. **2x instances**: ~$5-10/month (HA + capacity)

### Right-Sizing Process

```bash
# 1. Start with current config (1 GB)
fly deploy

# 2. Measure baseline
k6 run tests/load-test.js
fly status  # Note peak memory

# 3. Try 512 MB
# Edit fly.toml: memory_mb = 512
fly deploy

# 4. Test again
k6 run tests/load-test.js
fly logs | grep -i oom  # Check for kills

# 5. If stable for 48h → keep 512 MB
# If OOM or slow → revert to 1 GB
```

---

## Summary: Quick Commands

```bash
# Local testing
./test-local.sh
./test-docker.sh

# Load testing
k6 run tests/smoke-test.js
k6 run tests/load-test.js

# Deploy
fly deploy

# Monitor
fly status --app rust-c2s-api
fly logs -f --app rust-c2s-api

# Scale
fly scale memory 512  # Reduce memory
fly scale count 2     # Add instance
```

---

Generated: 2025-01-13
