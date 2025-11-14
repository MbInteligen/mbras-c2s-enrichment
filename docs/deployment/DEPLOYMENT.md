# Fly.io Deployment Guide

## Prerequisites

1. Install the Fly CLI:
```bash
curl -L https://fly.io/install.sh | sh
```

2. Login to Fly.io:
```bash
fly auth login
```

## Initial Setup

1. Create the app (first time only):
```bash
fly apps create rust-c2s-api --region gru
```

## Set Environment Variables

Set all required environment variables as secrets:

```bash
fly secrets set \
  DB_URL="postgresql://neondb_owner:password@host/database" \
  C2S_TOKEN="your-c2s-token" \
  C2S_BASE_URL="https://api.contact2sale.com" \
  WORK_API="your-work-api-key"
```

## Deploy

Deploy the application:

```bash
fly deploy
```

## Useful Commands

### View logs
```bash
fly logs
```

### Check app status
```bash
fly status
```

### Open the app
```bash
fly open
```

### SSH into the machine
```bash
fly ssh console
```

### Scale the app
```bash
# Scale to 2 machines
fly scale count 2

# Scale memory
fly scale memory 512
```

### Update secrets
```bash
fly secrets set KEY=value
```

### List secrets
```bash
fly secrets list
```

## Configuration

The app is configured via `fly.toml`:
- **Region**: gru (São Paulo, Brazil)
- **Port**: 8080 (internal)
- **Memory**: 1GB
- **Auto-scaling**: Enabled (min 0 machines when idle)
- **HTTPS**: Force enabled

## Health Check

The API includes a health endpoint:
```bash
curl https://rust-c2s-api.fly.dev/health
```

## Important Notes

1. **Never commit .env files** - All secrets are managed via `fly secrets`
2. **Database URL** - Make sure your Neon database allows connections from Fly.io
3. **Auto-stop** - Machines stop when idle to save costs, first request may be slower
4. **Logs** - Use `fly logs` to debug deployment issues
