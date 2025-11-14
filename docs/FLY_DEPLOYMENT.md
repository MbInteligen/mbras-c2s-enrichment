# Fly.io Deployment Guide

**App**: rust-c2s-api  
**Region**: GRU (São Paulo, Brazil)  
**Memory**: 256 MB  
**CPU**: Shared, 1 CPU

---

## Quick Deploy

```bash
# First time setup
fly launch

# Deploy updates
fly deploy

# Check status
fly status

# View logs
fly logs -f
```

---

## Memory Configuration

The app is configured to use **256 MB RAM** based on memory profiling:

- **Idle**: ~11 MB
- **Under Load**: ~17 MB
- **Safety Margin**: 15× headroom
- **Cost**: Most economical option

### Why 256 MB is Safe

Test results show peak memory usage of 17 MB even under load. With 256 MB:
- 239 MB available for spikes
- 10× average usage
- Room for OS overhead
- Supports 100+ concurrent requests

### If You Need More Memory

```bash
# Scale to 512 MB (safer, more expensive)
fly scale memory 512

# Scale to 1 GB (maximum safety)
fly scale memory 1024

# Check current allocation
fly status
```

---

## Environment Variables

**Required on Fly.io**:

```bash
# Set secrets (one-time)
fly secrets set C2S_TOKEN="your_c2s_token"
fly secrets set WORK_API="your_work_api_key"
fly secrets set DB_URL="postgresql://user:pass@host/db"
fly secrets set DIRETRIX_USER="your_username"
fly secrets set DIRETRIX_PASS="your_password"
fly secrets set DIRETRIX_BASE_URL="http://api.diretrixconsultoria.com.br"
fly secrets set C2S_BASE_URL="https://api.contact2sale.com"

# List current secrets
fly secrets list

# Unset a secret
fly secrets unset SECRET_NAME
```

**Note**: PORT=8080 is already set in fly.toml

---

## Auto-Start/Stop Configuration

Current settings (cost-saving):
```toml
auto_stop_machines = 'stop'      # Stop when idle
auto_start_machines = true       # Start on request
min_machines_running = 0         # No always-on instances
```

**Benefits**:
- Pay only when processing requests
- Automatically scales to zero
- Starts in <1 second on first request

**Trade-off**: First request after idle has ~1s cold start

### For Always-On (No Cold Starts)

Edit `fly.toml`:
```toml
min_machines_running = 1  # Keep 1 instance always running
```

Then deploy:
```bash
fly deploy
```

---

## Monitoring

### Real-time Status

```bash
# Overall health
fly status

# Live logs
fly logs -f

# Filter errors only
fly logs | grep ERROR

# Check memory usage
fly ssh console -C "free -h"
fly ssh console -C "ps aux | grep rust-c2s-api"
```

### Metrics to Watch

**Memory**:
- Normal: 10-20 MB
- Warning: >50 MB
- Critical: >150 MB (on 256 MB instance)

**Response Time**:
- Health check: <50ms
- Enrichment: <5s

**Error Rate**:
- Target: <1%
- Alert if: >5%

---

## Scaling Strategies

### Vertical Scaling (More Memory)

```bash
# Current: 256 MB
fly scale memory 256

# Moderate: 512 MB (recommended for production)
fly scale memory 512

# High: 1 GB (maximum safety)
fly scale memory 1024
```

**When to scale up**:
- Memory usage consistently >150 MB
- Frequent OOM errors
- High traffic (>100 req/min sustained)

### Horizontal Scaling (More Instances)

```bash
# Add instances
fly scale count 2  # 2 instances

# Scale by region
fly regions add iad  # Add US East
fly scale count 3    # 3 total instances
```

**When to scale out**:
- High availability required
- Traffic >200 req/min
- Multi-region deployment

---

## Regions

**Current**: GRU (São Paulo, Brazil)

### Add More Regions

```bash
# Add US East
fly regions add iad

# Add Europe
fly regions add lhr

# List available regions
fly platform regions

# See current regions
fly regions list
```

---

## Cost Optimization

### Current Configuration (256 MB, Auto-scale to Zero)

**Estimated Monthly Cost**: $1-3 USD
- Pay per request
- No idle costs
- Perfect for development/low traffic

### Production Configuration (512 MB, 1 Instance Always On)

**Estimated Monthly Cost**: $5-7 USD
- No cold starts
- Better for consistent traffic
- Recommended for production

### High-Availability (512 MB, 2 Instances, Multi-region)

**Estimated Monthly Cost**: $12-15 USD
- No downtime during deploys
- Geographic redundancy
- Automatic failover

---

## Deployment Checklist

### Pre-Deployment

- [ ] Update secrets on Fly.io (rotate from .env)
- [ ] Test locally: `cargo run`
- [ ] Run tests: `cargo test`
- [ ] Check formatting: `cargo fmt --check`
- [ ] Run lints: `cargo clippy`
- [ ] Build release: `cargo build --release`

### Deploy

```bash
# Deploy
fly deploy

# Verify
fly status
fly logs -f
curl https://your-app.fly.dev/health
```

### Post-Deployment

- [ ] Check health endpoint
- [ ] Test enrichment endpoint
- [ ] Verify logs (no errors)
- [ ] Monitor memory usage
- [ ] Test Make.com integration

---

## Troubleshooting

### Issue: Out of Memory

**Symptom**: App crashes, OOM errors in logs

**Solution**:
```bash
# Check memory usage
fly ssh console -C "free -h"

# Scale up
fly scale memory 512

# Or optimize application
# - Check for memory leaks
# - Reduce cache size
# - Limit concurrent requests
```

### Issue: Slow Cold Starts

**Symptom**: First request after idle takes >2s

**Solution**:
```bash
# Keep instance always running
fly scale count 1
```

Edit `fly.toml`:
```toml
min_machines_running = 1
```

### Issue: High Costs

**Symptom**: Unexpected billing

**Check**:
```bash
fly status
fly regions list
```

**Optimize**:
- Reduce memory allocation if overprovisioned
- Enable auto-stop if not needed 24/7
- Remove unused regions
- Reduce instance count

### Issue: Connection Errors

**Symptom**: Can't connect to app

**Check**:
```bash
# Is app running?
fly status

# Check logs
fly logs

# Test locally
curl https://your-app.fly.dev/health
```

**Fix**:
```bash
# Restart
fly deploy --force

# Or scale up from zero
fly scale count 1
```

---

## Security

### Secrets Management

**Never commit**:
- `.env` file
- Credentials in code
- API tokens in fly.toml

**Always use**:
```bash
fly secrets set KEY=value
```

### Rotate Credentials

```bash
# Update C2S token
fly secrets set C2S_TOKEN="new_token"

# Update database password
fly secrets set DB_URL="postgresql://user:new_pass@host/db"

# App automatically restarts with new secrets
```

---

## CI/CD Integration

### GitHub Actions

Create `.github/workflows/deploy.yml`:

```yaml
name: Deploy to Fly.io

on:
  push:
    branches: [main]

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: superfly/flyctl-actions/setup-flyctl@master
      - run: flyctl deploy --remote-only
        env:
          FLY_API_TOKEN: ${{ secrets.FLY_API_TOKEN }}
```

---

## Monitoring & Alerts

### Set Up Alerts

Fly.io doesn't have built-in alerting, but you can use:

1. **UptimeRobot** (free)
   - Monitor health endpoint
   - Email alerts on downtime

2. **Better Stack** (free tier)
   - Log aggregation
   - Error tracking
   - Slack notifications

3. **Custom Script**
   ```bash
   # Check health every 5 minutes
   */5 * * * * curl -f https://your-app.fly.dev/health || mail -s "App Down" you@example.com
   ```

---

## Backup & Recovery

### Database Backups

Your Neon database has automatic backups. For extra safety:

```bash
# Manual backup
pg_dump $DB_URL > backup-$(date +%Y%m%d).sql

# Restore
psql $DB_URL < backup-20251114.sql
```

### Configuration Backup

Keep fly.toml in version control:
```bash
git add fly.toml
git commit -m "Update Fly.io config"
git push
```

---

## Performance Tuning

### Current Optimizations

Already enabled in Cargo.toml:
```toml
[profile.release]
opt-level = 3        # Maximum optimization
lto = true           # Link-time optimization
codegen-units = 1    # Better optimization
strip = true         # Smaller binary
```

### If Performance Issues

1. **Check logs for slow queries**
   ```bash
   fly logs | grep "took"
   ```

2. **Monitor external API latency**
   - Work API
   - Diretrix API
   - C2S API

3. **Consider caching**
   - Already implemented for CPF deduplication
   - Consider adding more caching if needed

---

## Current Configuration Summary

```toml
App Name:     rust-c2s-api
Region:       GRU (São Paulo)
Memory:       256 MB
CPU:          Shared, 1 CPU
Auto-stop:    Yes (save costs)
Auto-start:   Yes
Min Running:  0 (scales to zero)
Port:         8080
HTTPS:        Forced
```

**Status**: Optimized for cost and performance ✅

---

## Next Steps

1. **First Deployment**:
   ```bash
   fly launch
   fly secrets set C2S_TOKEN="..." WORK_API="..." DB_URL="..."
   fly deploy
   ```

2. **Update Make.com**:
   - URL: `https://your-app.fly.dev/api/v1/leads/process?id={{lead.id}}`
   - Method: GET

3. **Monitor**:
   ```bash
   fly logs -f
   ```

4. **Test**:
   ```bash
   curl https://your-app.fly.dev/health
   ```

---

**Status**: Ready for production deployment with 256 MB! 🚀
