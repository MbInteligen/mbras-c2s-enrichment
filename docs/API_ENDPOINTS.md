# API Endpoints Reference

## Overview

The rust-c2s-api provides endpoints for customer enrichment through Work API integration and database lookups.

**Base URL**: `http://localhost:3000` (configurable via `PORT` env var)

---

## Core Endpoints

### 1. Health Check

Check if the API is running.

```http
GET /health
```

**Response:**
```json
{
  "status": "healthy",
  "service": "rust-c2s-api",
  "version": "0.1.0"
}
```

**Example:**
```bash
curl http://localhost:3000/health
```

---

### 2. Get Customer (Main Enrichment Endpoint)

Retrieve and enrich customer data. This endpoint is called by mbras-c2s.

```http
GET /api/v1/contributor/customer
```

**Query Parameters:**
- `cpf` (optional) - Customer CPF (11 digits)
- `email` (optional) - Customer email
- `phone` (optional) - Customer phone
- `name` (optional) - Customer name

*At least one parameter is required.*

**Response:**
```json
{
  "source": "rust-c2s-api",
  "type": "customer",
  "personal_info": {
    "cpf": "12345678901",
    "name": "João Silva",
    "birth_date": "1990-01-01",
    "gender": "M",
    "mother_name": "Maria Silva",
    "father_name": "José Silva",
    "marital_status": null,
    "rg": "123456789",
    "voter_id": "123456789012"
  },
  "contact_info": {
    "emails": [
      {
        "email": "joao@example.com",
        "is_valid": true,
        "source": "work_api"
      }
    ],
    "phones": [
      {
        "phone": "11987654321",
        "ddd": "11",
        "operator": "Vivo",
        "type": "mobile",
        "is_valid": true,
        "source": "work_api"
      }
    ]
  },
  "addresses": [
    {
      "street": "Rua das Flores",
      "number": "123",
      "complement": "Apto 45",
      "neighborhood": "Centro",
      "city": "São Paulo",
      "state": "SP",
      "cep": "01234567",
      "source": "work_api"
    }
  ],
  "financial_info": null,
  "interests": null,
  "metadata": {
    "enriched": true,
    "sources": ["local_db", "work_api"],
    "timestamp": "2025-01-13T20:00:00Z",
    "modules_consulted": ["cpf", "telefone", "email", "cep", "mae", "titulo"]
  }
}
```

**Examples:**
```bash
# Query by CPF
curl "http://localhost:3000/api/v1/contributor/customer?cpf=12345678901"

# Query by email
curl "http://localhost:3000/api/v1/contributor/customer?email=user@example.com"

# Query by phone
curl "http://localhost:3000/api/v1/contributor/customer?phone=11987654321"
```

**Flow:**
1. Search local PostgreSQL database
2. If found but not enriched → call Work API
3. If not found → call Work API directly
4. Return unified response

---

### 3. Get Customer by ID

Retrieve customer by UUID from database.

```http
GET /api/v1/customers/{uuid}
```

**Path Parameters:**
- `uuid` - Customer UUID

**Response:**
```json
{
  "customer": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "party_type": "customer",
    "cpf_cnpj": "12345678901",
    "full_name": "João Silva",
    "birth_date": "1990-01-01",
    "sex": "M",
    "enriched": true,
    "created_at": "2025-01-13T20:00:00Z"
  },
  "emails": [...],
  "phones": [...],
  "enrichment_data": null
}
```

**Example:**
```bash
curl "http://localhost:3000/api/v1/customers/550e8400-e29b-41d4-a716-446655440000"
```

---

### 4. Enrich Customer

Explicitly trigger enrichment for a customer.

```http
POST /api/v1/enrich
```

**Query Parameters:**
Same as `/api/v1/contributor/customer`

**Response:**
Same as `/api/v1/contributor/customer`

**Example:**
```bash
curl -X POST "http://localhost:3000/api/v1/enrich?cpf=12345678901"
```

---

### 5. Process Lead

Process a lead submission (similar to mbras-c2s flow).

```http
POST /api/v1/leads
```

**Request Body:**
```json
{
  "lead_id": "12345",
  "personal_info": {
    "name": "João Silva",
    "cpf": "12345678901",
    "email": "joao@example.com"
  },
  "contact_info": {
    "phones": [
      {
        "phone": "11987654321"
      }
    ]
  }
}
```

**Response:**
```json
{
  "success": true,
  "message": "Lead processed and enriched successfully",
  "data": {
    "customer": {...},
    "emails": [...],
    "phones": [...]
  }
}
```

**Example:**
```bash
curl -X POST http://localhost:3000/api/v1/leads \
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
  }'
```

---

## Work API Integration Endpoints

### 6. Fetch All Work API Modules

Query all available Work API modules for a document.

```http
GET /api/v1/work/modules/all
```

**Query Parameters:**
- `documento` (required) - CPF, CNPJ, email, phone, etc.

**Response:**
```json
{
  "telefone": {
    "status": "success",
    "data": [...]
  },
  "cpf": {
    "status": "success",
    "data": {...}
  },
  "nome": {
    "status": "success",
    "data": {...}
  },
  "email": {
    "status": "success",
    "data": [...]
  },
  "titulo": {
    "status": "success",
    "data": {...}
  },
  "cep": {
    "status": "success",
    "data": {...}
  },
  "mae": {
    "status": "success",
    "data": {...}
  },
  "cnpj": {
    "status": "success",
    "data": {...}
  }
}
```

**Example:**
```bash
curl "http://localhost:3000/api/v1/work/modules/all?documento=12345678901"
```

**Note:** This endpoint requires that your Work API token has purchased access to the modules.

---

### 7. Fetch Specific Work API Module

Query a single Work API module.

```http
GET /api/v1/work/modules/{module}
```

**Path Parameters:**
- `module` - Module name (tel, cpf, nome, email, titulo, cep, mae, cnpj)

**Query Parameters:**
- `documento` (required) - Document to query

**Response:**
Varies by module. Example for CPF module:
```json
{
  "cpf": "12345678901",
  "nome": "João Silva",
  "nascimento": "1990-01-01",
  "sexo": "M",
  "mae": "Maria Silva",
  "rg": "123456789"
}
```

**Examples:**
```bash
# Get phone data
curl "http://localhost:3000/api/v1/work/modules/tel?documento=12345678901"

# Get CPF data
curl "http://localhost:3000/api/v1/work/modules/cpf?documento=12345678901"

# Get email data
curl "http://localhost:3000/api/v1/work/modules/email?documento=nome@example.com"

# Get address by CEP
curl "http://localhost:3000/api/v1/work/modules/cep?documento=01310100"
```

---

## Work API Modules Reference

Based on the screenshot provided, these are the available modules once purchased:

| Module    | Cost      | Input        | Returns                              |
|-----------|-----------|--------------|--------------------------------------|
| `tel`     | R$ 125,00 | CPF/Name     | Phone numbers, DDD, operator, type   |
| `cpf`     | R$ 125,00 | CPF          | Name, birth date, gender, RG, mother |
| `nome`    | R$ 125,00 | Name         | CPF, variations                      |
| `email`   | R$ 125,00 | CPF/Name     | Email addresses with validation      |
| `titulo`  | R$ 125,00 | CPF          | Voter ID (Título de Eleitor)         |
| `cep`     | R$ 125,00 | CPF/CEP      | Full address details                 |
| `mae`     | R$ 125,00 | CPF          | Mother's full name                   |
| `cnpj`    | R$ 100,00 | CNPJ         | Company data                         |

**Total for all modules**: R$ 975,00

---

## Error Responses

All endpoints may return these error responses:

### 400 Bad Request
```json
{
  "error": "Missing required parameters"
}
```

### 404 Not Found
```json
{
  "error": "Customer not found in database or Work API"
}
```

### 500 Internal Server Error
```json
{
  "error": "Internal server error"
}
```

### 502 Bad Gateway
```json
{
  "error": "External service error"
}
```

---

## Authentication

Currently, the API does not require authentication. For production use, consider adding:
- API key authentication
- JWT tokens
- Rate limiting per key

---

## Rate Limiting

No rate limiting is currently implemented. Consider implementing based on:
- Work API costs (R$ 975,00 per enrichment)
- Database load
- API usage patterns

---

## Integration with mbras-c2s

Configure mbras-c2s to use this API:

```env
LOOKUP_API_URL=http://localhost:3000/api/v1
C2S_TOKEN=your_token_here
```

The main endpoint `/api/v1/contributor/customer` is compatible with the mbras-c2s `LookupResponse` model.

---

## Response Format Notes

### Data Sources

Each response includes metadata about data sources:

- `"local_db"` - Data from PostgreSQL database
- `"work_api"` - Data from Work API enrichment

### Enrichment Flag

The `metadata.enriched` flag indicates whether Work API was called:
- `true` - Work API data included
- `false` - Database-only response

### Modules Consulted

The `metadata.modules_consulted` array shows which Work API modules were queried.

---

## Testing

See the test scripts:
- `test_all_modules.sh` - Full integration test
- `test_direct_work_api.sh` - Direct Work API testing
- `test_modules.sh` - Individual module testing

---

## Running the Server

```bash
# Development
cargo run

# Production (optimized build)
cargo build --release
./target/release/rust-c2s-api
```

Server will start on the port specified in `.env` (default: 3000).

---

## Logs

Set log level with `RUST_LOG` environment variable:

```bash
RUST_LOG=debug cargo run    # Detailed logs
RUST_LOG=info cargo run     # Standard logs
RUST_LOG=warn cargo run     # Warnings only
```
