# Deploy to Fly.io - Step by Step

Follow these steps **one at a time** to deploy your app.

---

## Step 1: Create the Fly.io App

```bash
cd /Users/ronaldo/Documents/GitHub/GO/rust-c2s-api
fly launch --no-deploy
```

**What happens**: 
- Creates app on Fly.io
- Updates `fly.toml` with app name
- Does NOT deploy yet (we need to set secrets first)

**Expected output**: 
```
Created app 'rust-c2s-api' in organization 'personal'
Admin URL: https://fly.io/apps/rust-c2s-api
Hostname: rust-c2s-api.fly.dev
```

---

## Step 2: Set Secrets (IMPORTANT)

⚠️ **Use your ROTATED credentials here, not the ones in .env!**

```bash
# Set C2S token
fly secrets set C2S_TOKEN="your_new_c2s_token"

# Set Work API key
fly secrets set WORK_API="your_new_work_api_key"

# Set database URL
fly secrets set DB_URL="postgresql://user:new_password@host/db?sslmode=require"

# Set Diretrix credentials
fly secrets set DIRETRIX_USER="your_user"
fly secrets set DIRETRIX_PASS="your_new_password"

# Set API URLs
fly secrets set DIRETRIX_BASE_URL="http://api.diretrixconsultoria.com.br"
fly secrets set C2S_BASE_URL="https://api.contact2sale.com"
```

**Check your secrets**:
```bash
fly secrets list
```

---

## Step 3: Deploy the App

```bash
fly deploy
```

**What happens**:
- Builds Docker image
- Deploys to Fly.io
- Starts the app
- Takes 2-5 minutes

**Expected output**:
```
==> Building image
...
--> Pushing image done
==> Deploying
...
✓ Deployment successful!
```

---

## Step 4: Verify Deployment

```bash
# Check status
fly status

# Test health endpoint
curl https://rust-c2s-api.fly.dev/health

# View logs
fly logs
```

**Expected response from /health**:
```json
{
  "status": "healthy",
  "service": "rust-c2s-api",
  "version": "0.1.0"
}
```

---

## Step 5: Test Enrichment

```bash
# Test with a real lead ID
curl "https://rust-c2s-api.fly.dev/api/v1/leads/process?id=YOUR_LEAD_ID"
```

**Expected response**:
```json
{
  "success": true,
  "message": "Successfully processed and enriched lead...",
  "lead_id": "...",
  "cpfs_processed": ["..."],
  "entities_stored": 1
}
```

---

## Troubleshooting

### Error: "Could not find App"
**Solution**: Run `fly launch --no-deploy` first

### Error: "missing secrets"
**Solution**: Set all secrets (Step 2) before deploying

### Error: "deployment failed"
**Check logs**:
```bash
fly logs
```

### Error: "connection refused"
**Wait a moment**, app might be starting:
```bash
fly status  # Check if app is running
```

---

## Common Commands

```bash
# View logs (real-time)
fly logs

# Check app status
fly status

# Restart app
fly apps restart rust-c2s-api

# SSH into machine
fly ssh console

# Scale memory (if needed)
fly scale memory 512

# Update secrets
fly secrets set KEY=new_value
```

---

## Update Make.com

After successful deployment, update Make.com webhook:

**URL**: `https://rust-c2s-api.fly.dev/api/v1/leads/process?id={{lead.id}}`  
**Method**: GET

---

## If You Get Stuck

1. Check logs: `fly logs`
2. Check status: `fly status`
3. Verify secrets: `fly secrets list`
4. Test health: `curl https://rust-c2s-api.fly.dev/health`

---

**Ready?** Start with Step 1! 🚀
