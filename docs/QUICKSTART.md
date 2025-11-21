# Quick Start Guide - Rust C2S API

## 🚀 Getting Started in 5 Minutes

### Step 1: Verify Environment

Check that `.env` file exists with all credentials:

```bash
cat .env
```

Should contain:
- `C2S_TOKEN` - Your Contact2Sale API token
- `C2S_BASE_URL` - Contact2Sale API base URL
- `WORK_API` - Work API token (completa.workbuscas.com)
- `DB_URL` - PostgreSQL connection string
- `PORT` - Server port (default: 8080)

### Step 2: Build the Project

```bash
cargo build --release
```

This compiles an optimized production binary.

### Step 3: Run the Server

```bash
cargo run --release
```

Or run the binary directly:

```bash
./target/release/rust-c2s-api
```

You should see:
```
2025-01-13T20:00:00.000000Z  INFO rust_c2s_api: Configuration loaded successfully
2025-01-13T20:00:00.000000Z  INFO rust_c2s_api: Database connection pool established
2025-01-13T20:00:00.000000Z  INFO rust_c2s_api: Server listening on 0.0.0.0:8080
```

### Step 4: Test the API

Open a new terminal and test:

```bash
# Health check
curl http://localhost:8080/health

# Get a customer from your database
curl "http://localhost:8080/api/v1/contributor/customer?cpf=YOUR_CPF_HERE" | jq '.'
```

## 🧪 Running Tests

### Test Work API Modules

```bash
./test_modules.sh
```

This script will:
1. Fetch a test person from your database
2. Test each Work API module individually
3. Test all modules combined
4. Display detailed results for each module

### Expected Output

```
==========================================
Work API Module Testing
==========================================
Token: zuZKCfxQqG...
Base URL: https://completa.workbuscas.com

Testing module: tel
------------------------------------------
✅ Response received
{
  "telefone": {
    "status": "success",
    "data": [...]
  }
}

Testing module: cpf
------------------------------------------
✅ Response received
{
  "cpf": {
    "status": "success",
    "data": {...}
  }
}

... (continues for all modules)
```

## 📊 Understanding the Response

When you query `/api/v1/contributor/customer`, you get a unified response:

```json
{
  "source": "rust-c2s-api",
  "type": "customer",
  "personal_info": {
    "cpf": "...",
    "name": "...",
    "birth_date": "...",
    "gender": "...",
    "mother_name": "...",
    "father_name": "...",
    "rg": "...",
    "voter_id": "..."
  },
  "contact_info": {
    "emails": [...],
    "phones": [...]
  },
  "addresses": [...],
  "metadata": {
    "enriched": true,
    "sources": ["local_db", "work_api"],
    "timestamp": "2025-01-13T20:00:00Z",
    "modules_consulted": ["cpf", "telefone", "email", "cep", "mae", "titulo"]
  }
}
```

### Key Fields

- **`metadata.enriched`**: Whether Work API enrichment was performed
- **`metadata.sources`**: Where data came from (`local_db`, `work_api`)
- **`metadata.modules_consulted`**: Which Work API modules were called
- **Contact info sources**: Each email/phone has a `source` field indicating origin

## 🔗 Integration with mbras-c2s

### Configuration

In your mbras-c2s `.env` or configuration:

```env
LOOKUP_API_URL=http://rust-c2s-api:8080/api/v1
C2S_TOKEN=your_c2s_token_here
```

### How It Works

1. **mbras-c2s** receives a lead
2. Calls `ProcessLead` function
3. Makes GET request to `http://rust-c2s-api:8080/api/v1/contributor/customer?cpf=...`
4. **rust-c2s-api**:
   - Searches local database
   - If not enriched, calls Work API (all 8 modules)
   - Returns unified response
5. **mbras-c2s** validates response
6. If valid, forwards to Contact2Sale

## 💰 Cost Tracking

Each Work API enrichment call queries **8 modules**:

| Module  | Cost      |
|---------|-----------|
| tel     | R$ 125,00 |
| cpf     | R$ 125,00 |
| nome    | R$ 125,00 |
| email   | R$ 125,00 |
| titulo  | R$ 125,00 |
| cep     | R$ 125,00 |
| mae     | R$ 125,00 |
| cnpj    | R$ 100,00 |
| **Total** | **R$ 975,00** |

💡 **Tip**: The API caches enriched data in the database (sets `enriched = true`), so subsequent queries won't trigger new Work API calls.

## 🐛 Troubleshooting

### Server won't start

1. Check database connection:
   ```bash
   psql "$DB_URL" -c "SELECT 1"
   ```

2. Verify port is available:
   ```bash
   lsof -i :8080
   ```

3. Check environment variables:
   ```bash
   cargo run 2>&1 | grep -i "error"
   ```

### Work API not returning data

1. Verify token:
   ```bash
   curl "https://completa.workbuscas.com/api?token=$WORK_API&modulo=cpf&consulta=12345678901"
   ```

2. Check logs for errors:
   ```bash
   RUST_LOG=debug cargo run
   ```

### Database queries failing

1. Test database connection:
   ```bash
   psql "$DB_URL" -c "SELECT COUNT(*) FROM core.parties"
   ```

2. Check schema exists:
   ```bash
   psql "$DB_URL" -c "\dn"
   ```

## 📝 Common Use Cases

### Query by CPF

```bash
curl "http://localhost:8080/api/v1/contributor/customer?cpf=12345678901" | jq '.'
```

### Query by Email

```bash
curl "http://localhost:8080/api/v1/contributor/customer?email=user@example.com" | jq '.'
```

### Query by Phone

```bash
curl "http://localhost:8080/api/v1/contributor/customer?phone=11987654321" | jq '.'
```

### Process a Lead

```bash
curl -X POST http://localhost:8080/api/v1/leads \
  -H "Content-Type: application/json" \
  -d '{
    "lead_id": "test-123",
    "personal_info": {
      "name": "João Silva",
      "cpf": "12345678901",
      "email": "joao@example.com"
    },
    "contact_info": {
      "phones": [{"phone": "11987654321"}]
    }
  }' | jq '.'
```

## 🚢 Deployment

### Docker

```bash
docker-compose up -d
```

### Checking Logs

```bash
# Docker
docker-compose logs -f

# Binary
./target/release/rust-c2s-api 2>&1 | tee api.log
```

### Environment Variables in Production

Make sure to set:
- `RUST_LOG=info` (or `warn` for less verbosity)
- All credentials in `.env` or environment

## 🎯 Next Steps

1. **Monitor enrichment costs**: Track how many Work API calls are made
2. **Optimize caching**: Ensure `enriched = true` is set after enrichment
3. **Add metrics**: Consider adding Prometheus/Grafana for monitoring
4. **Rate limiting**: Add rate limiting if needed to control Work API costs

## 📚 Additional Resources

- [Full API Documentation](README.md)
- Work API docs: https://completa.workbuscas.com
- Contact2Sale API docs: https://api.contact2sale.com

## ✅ Checklist

- [ ] `.env` file created with all credentials
- [ ] Database accessible and schema created
- [ ] Application builds successfully
- [ ] Server starts and health endpoint responds
- [ ] Work API token valid and returning data
- [ ] Integration with mbras-c2s configured
- [ ] Test scripts executed successfully

---

**Need help?** Check the logs with `RUST_LOG=debug cargo run` for detailed debugging information.
