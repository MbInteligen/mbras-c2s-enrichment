# AI Endpoints Guide — rust-c2s-api

> Comprehensive reference for agents and frontends consuming the MBRAS C2S AI endpoints.
>
> **Base URL:** `https://mbras-c2s.fly.dev`
> **Last updated:** February 16, 2026

---

## Table of Contents

- [Overview](#overview)
- [Authentication](#authentication)
- [Endpoints](#endpoints)
  - [POST /api/v1/ai/interpret](#post-apiv1aiinterpret)
  - [POST /api/v1/ai/generate](#post-apiv1aigenerate)
  - [GET /api/v1/ai/models](#get-apiv1aimodels)
- [Model Routing](#model-routing)
- [Command Reference](#command-reference)
- [Content Templates](#content-templates)
- [Customer Context Schema](#customer-context-schema)
- [Brand Guardrails](#brand-guardrails)
- [Internal Knowledge Base](#internal-knowledge-base)
- [Integration Patterns](#integration-patterns)
- [Error Handling](#error-handling)
- [Rate Limits and Timeouts](#rate-limits-and-timeouts)

---

## Overview

The AI layer provides three endpoints that sit on top of the existing C2S enrichment API:

| Endpoint | Purpose | Default Model | Temperature |
|----------|---------|---------------|-------------|
| `POST /api/v1/ai/interpret` | Natural language → structured command | `base` (Gemini Flash) | 0.1 |
| `POST /api/v1/ai/generate` | Customer context → brand-compliant content | `smart` (Claude Opus) | 0.7 |
| `GET /api/v1/ai/models` | List available model tiers | — | — |

All AI requests are proxied through OpenRouter. The API key is server-side only — clients never see it.

---

## Authentication

No authentication is required for the AI endpoints. The OpenRouter API key is managed server-side via the `OPENROUTER_API_KEY` environment variable.

Rate limiting is enforced at 10 requests/second (tower-governor).

---

## Endpoints

### POST /api/v1/ai/interpret

Converts natural language input into a structured CRM command. Supports conversational follow-ups via optional message history.

**Request:**

```json
{
  "input": "busca o CPF 123.456.789-01",
  "model": "base",
  "messages": [
    { "role": "user", "content": "quem é o dono do telefone 11999887766?" },
    { "role": "assistant", "content": "Encontrei João Silva, CPF 12345678901..." }
  ]
}
```

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `input` | string | Yes | — | Natural language input from the user |
| `model` | string | No | `"base"` | Model tier: `"fast"`, `"base"`, or `"smart"` |
| `messages` | array | No | `[]` | Conversation history (last 5-10 messages) for contextual follow-ups |

**Message object:**

| Field | Type | Description |
|-------|------|-------------|
| `role` | string | `"user"` or `"assistant"` |
| `content` | string | Message text (stripped of raw JSON, only formatted text) |

**Response (200):**

```json
{
  "command": "cpf",
  "args": "12345678901"
}
```

The response is always one of two modes:

**Mode 1 — Command** (execute a CRM action):
```json
{ "command": "cpf", "args": "12345678901" }
```

**Mode 2 — Chat** (conversational response):
```json
{ "command": "chat", "args": "Olá! Sou o assistente MBRAS..." }
```

The AI decides which mode based on user intent. Greetings, questions about capabilities, and clarifying questions return `chat`. Clear data lookups or actions return a specific command.

**Conversation Context:**

When `messages[]` is provided, the AI can resolve follow-up references:
- "enrich him" → resolves to the CPF/phone from the previous message
- "draft a WhatsApp for this client" → uses the customer data from context
- "make it shorter" → rewrites the previous generated content
- Maximum 10 messages are used (older ones are trimmed)

**PII Note:** Strip `raw_json` from messages before sending. Only send formatted text and key fields.

---

### POST /api/v1/ai/generate

Generates brand-compliant content using customer context. Uses higher temperature (0.7) and longer output (2048 tokens) than interpret.

**Request:**

```json
{
  "template": "whatsapp_reply",
  "context": {
    "personal_info": {
      "name": "João Silva",
      "gender": "M",
      "marital_status": "Casado"
    },
    "financial_info": {
      "estimated_income": 150000,
      "credit_score": 850,
      "income_range": "R$ 100K-200K"
    },
    "wealth_assessment": {
      "tier": "A",
      "tier_label": "Alto Patrimônio",
      "estimated_net_worth": 15000000
    },
    "contact_info": {
      "phones": [{ "phone": "11999887766" }],
      "emails": [{ "email": "joao@email.com" }]
    },
    "addresses": [
      { "neighborhood": "Itaim Bibi", "city": "São Paulo" }
    ],
    "metadata": {
      "sources": ["db", "Work API", "IBVI"]
    }
  },
  "instructions": "Mencionar o apartamento no Itaim que ele visitou semana passada",
  "model": "smart"
}
```

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `template` | string | Yes | — | One of: `whatsapp_reply`, `email_draft`, `instagram_caption`, `listing_description`, `reel_script` |
| `context` | object | Yes | — | Customer data (see [Customer Context Schema](#customer-context-schema)) |
| `instructions` | string | No | — | Additional instructions for the AI |
| `model` | string | No | `"smart"` | Model tier: `"fast"`, `"base"`, or `"smart"` |

**Response (200):**

```json
{
  "content": "Olá, João! Tudo bem?\n\nFiquei feliz com sua visita...",
  "template": "WhatsApp Reply",
  "model": "anthropic/claude-opus-4.6",
  "tokens_used": 187
}
```

| Field | Type | Description |
|-------|------|-------------|
| `content` | string | Generated text, ready to copy-paste |
| `template` | string | Human-readable template label |
| `model` | string | Actual model ID used |
| `tokens_used` | number or null | Total tokens consumed (if reported by provider) |

---

### GET /api/v1/ai/models

Returns the available model tiers and their current configurations.

**Response (200):**

```json
[
  { "id": "fast", "label": "Fast", "model": "moonshotai/kimi-k2.5" },
  { "id": "base", "label": "Base", "model": "google/gemini-3-flash-preview" },
  { "id": "smart", "label": "Smart", "model": "anthropic/claude-opus-4.6" }
]
```

Models are configurable via environment variables (`MODEL_FAST`, `MODEL_BASE`, `MODEL_SMART`).

---

## Model Routing

| Tier | Default Model | Use Case | Temp | Max Tokens |
|------|---------------|----------|------|------------|
| `fast` | `moonshotai/kimi-k2.5` | Quick routing, simple commands | 0.1 | 256 |
| `base` | `google/gemini-3-flash-preview` | Command interpretation (default for interpret) | 0.1 | 256 |
| `smart` | `anthropic/claude-opus-4.6` | Content generation (default for generate) | 0.7 | 2048 |

**Recommendation:** Let each endpoint use its default. Override only when needed:
- Use `"fast"` for high-volume interpret calls where latency matters
- Use `"smart"` for interpret only when complex reasoning is needed
- Use `"base"` for generate only when cost matters more than quality

---

## Command Reference

These are the commands returned by `/api/v1/ai/interpret`. Each maps to an existing API endpoint:

### Person Lookup

| Command | Args | API Endpoint |
|---------|------|-------------|
| `cpf` | CPF (11 digits) | `GET /api/v1/work/modules/all?documento={cpf}` |
| `name` | Full name | `GET /api/v1/contributor/search?q={name}` |
| `phone` | Phone (10-11 digits) | `GET /api/v1/contributor/customer?phone={phone}` |
| `email` | Email address | `GET /api/v1/contributor/customer?email={email}` |
| `customer` | UUID | `GET /api/v1/customers/{id}` |

### Enrichment

| Command | Args | API Endpoint |
|---------|------|-------------|
| `enrich` | CPF or phone | `POST /api/v1/enrich` |
| `work` | `module value` | `GET /api/v1/work/modules/{module}?documento={value}` |
| `batch` | `cpf,cpf,...` | `POST /batch/enrich-direct` |
| `batch-retry` | — | `POST /batch/retry-failed` |

### Companies

| Command | Args | API Endpoint |
|---------|------|-------------|
| `company` | CPF (11 digits) | `GET /api/v1/company/cpf/{cpf}` |
| `cnpj` | CNPJ (14 digits) | `GET /api/v1/company/cnpj/{cnpj}` |
| `search` | Query text | `GET /api/v1/company/search?q={query}` |

### Properties

| Command | Args | API Endpoint |
|---------|------|-------------|
| `property` | CPF (11 digits) | `GET /api/v1/property/cpf/{cpf}` |

### Analysis

| Command | Args | API Endpoint |
|---------|------|-------------|
| `analyze` | Lead ID | `POST /api/v1/analyze/{lead_id}` |
| `get-analysis` | Lead ID | `GET /api/v1/analysis/{lead_id}` |

### C2S CRM

| Command | Args | API Endpoint |
|---------|------|-------------|
| `sellers` | — | C2S API: list sellers |
| `seller` | Seller ID | C2S API: seller details |
| `find-phone` | Phone | C2S API: find lead by phone |
| `find-email` | Email | C2S API: find lead by email |
| `lead-status` | Lead ID | C2S API: lead enrichment status |
| `forward` | `lead_id seller_id` | C2S API: forward lead |
| `interact` | Lead ID | C2S API: mark as interacted |
| `tags` | — | C2S API: list tags |
| `create-tag` | Tag name | C2S API: create tag |
| `lead-tags` | Lead ID | C2S API: lead tags |
| `tag` | `lead_id tag_name` | C2S API: add tag to lead |

### Activities

| Command | Args | API Endpoint |
|---------|------|-------------|
| `note` | `lead_id text` | C2S API: add note |
| `call` | Lead ID | C2S API: register call |
| `email-lead` | `lead_id text` | C2S API: register email |
| `meeting` | Lead ID | C2S API: register meeting |
| `task` | `lead_id description` | C2S API: create task |

### Distribution

| Command | Args | API Endpoint |
|---------|------|-------------|
| `distribute` | — | C2S API: distribute leads |
| `auto-assign` | — | C2S API: auto-assign leads |

### Twenty CRM

| Command | Args | API Endpoint |
|---------|------|-------------|
| `pipeline` | — | `GET /twenty/stats/pipeline` |
| `broker-stats` | — | `GET /twenty/stats/broker` |
| `sla` | — | `GET /twenty/sla/violations` |
| `lead-action` | Lead ID | `GET /twenty/leads/{id}/next-action` |
| `lead-sla` | Lead ID | SLA status for lead |

### Monitoring

| Command | Args | API Endpoint |
|---------|------|-------------|
| `health` | — | `GET /health` |
| `stats` | — | `GET /stats/enrichment` |
| `services` | — | `GET /stats/health` |
| `dashboard` | — | Dashboard data |
| `help` | — | Local (returns command list) |

### Chat (conversational)

| Command | Args | Description |
|---------|------|-------------|
| `chat` | Message text | Conversational response — not a CRM action |

---

## Content Templates

### whatsapp_reply (max 500 chars)
- Short, confident, high-end tone
- 1-2 message variants (separated by `---`)
- Must include a CTA (visit, call, meeting)
- May use 1-2 tasteful emojis max
- Ready to copy-paste into WhatsApp

### email_draft (max 2000 chars)
- Subject line (max 60 chars) + body
- Structure: greeting → context → main content → CTA → signature placeholder
- Includes `[CRECI XXXXX]` placeholder in signature
- Formal luxury tone

### instagram_caption (max 2200 chars)
- Structure: HOOK → BODY (2-3 paragraphs) → CTA → HASHTAGS
- 15-20 hashtags (branded + location + niche)
- May include 3-5 tasteful emojis between sections

### listing_description (max 2000 chars)
- Structure: HEADLINE → NARRATIVE → SPECS → DIFFERENTIATORS → NEIGHBORHOOD
- Specs listed only from provided data
- Missing specs marked as `[DADOS INDISPONÍVEIS]`

### reel_script (max 1000 chars)
- 15-60 second duration with timestamps
- Structure: HOOK (0-3s) → BODY (4-50s) → CTA (last 5-10s)
- Includes camera/visual directions in `[brackets]`

---

## Customer Context Schema

The `context` field for `/api/v1/ai/generate` accepts a JSON object. The AI extracts a readable summary from these fields:

```json
{
  "personal_info": {
    "name": "string",
    "gender": "M | F",
    "marital_status": "string"
  },
  "financial_info": {
    "estimated_income": 150000.0,
    "income": 150000.0,
    "credit_score": 850.0,
    "income_range": "string"
  },
  "wealth_assessment": {
    "tier": "S | A | B | C",
    "tier_label": "string",
    "estimated_net_worth": 15000000.0
  },
  "contact_info": {
    "phones": [{ "phone": "11999887766" }],
    "emails": [{ "email": "user@example.com" }]
  },
  "addresses": [
    { "neighborhood": "Itaim Bibi", "city": "São Paulo" }
  ],
  "metadata": {
    "sources": ["db", "Work API", "IBVI", "Meilisearch", "Diretrix"]
  }
}
```

**Tip:** This matches the `UnifiedCustomerResponse` returned by `GET /api/v1/contributor/customer`. You can pass the full response directly — the AI extracts only what it needs.

All fields are optional. Missing data is labeled `[DADOS INDISPONÍVEIS]` in the generated content.

---

## Brand Guardrails

All generated content follows MBRAS brand rules enforced via system prompt:

### Voice Rules
- Confident and exclusive, never salesy or desperate
- PT-BR formal register (no slang, no diminutives)
- Luxury vocabulary: "exclusivo", "sofisticado", "alto padrão", "requintado"
- Short, impactful sentences
- Address client with "você" (formal), never "tu"

### Forbidden Content
- Unverified superlatives: "o melhor de São Paulo", "incomparável"
- Investment promises: "valorização garantida", "retorno certo"
- Misleading claims about area, price, or specs
- Competitor mentions
- Generic clichés: "oportunidade única", "não perca", "imperdível"
- Excessive emojis (WhatsApp: max 2, email/listing: none)

### Required Elements
- Clear CTA in every outbound message
- CRECI reference in formal content (email, listing)
- Neighborhood name when a property is referenced
- Missing data marked as `[DADOS INDISPONÍVEIS]`
- Warning "⚠ Confirme os dados antes de enviar ao cliente." when critical data is missing

### Data Integrity
- Only uses data provided in the context — never fabricates
- Numerical claims (price, area, R$/m²) must come from the context
- Language: always PT-BR unless explicitly asked for English

---

## Internal Knowledge Base

The interpret endpoint embeds MBRAS operational knowledge in its system prompt. When users ask about company procedures, the AI answers from this knowledge base using `chat` mode.

### Topics Covered

| Topic | Key Information |
|-------|-----------------|
| **About MBRAS** | Luxury brokerage in São Paulo, focus neighborhoods |
| **Lead Tiers** | S (>R$50M, 24h SLA), A (R$10-50M, 48h), B (R$3-10M, 72h), C (<R$3M, 72h) |
| **Lead Lifecycle** | 7 steps: webhook → enrichment → tier → distribution → contact → activities → pipeline |
| **SLA Rules** | Tier-based first contact deadlines, violation alerts |
| **Enrichment Process** | CPF discovery → Work API → company/property lookup, score 0-1000 |
| **Top 10 Neighborhoods** | Itaim Bibi, Vila Nova Conceição, Jardim Paulista, Moema, Jardim Europa, Cidade Jardim, Vila Olímpia, Cerqueira César, Jardim Paulistano, Pinheiros |
| **Broker Operations** | sellers, forward, distribute, auto-assign, interact commands |
| **Tag System** | Lead categorization (Alto Padrão, Investidor, Locação) |
| **Common Q&A** | How to forward leads, check SLA, enrich, understand credit score, register calls |

---

## Integration Patterns

### Pattern 1: Simple Router

Use interpret to convert user input into API calls:

```
User input → POST /api/v1/ai/interpret → { command, args }
                                           ↓
                                    Execute via API endpoint
```

```bash
# Step 1: Interpret
curl -X POST https://mbras-c2s.fly.dev/api/v1/ai/interpret \
  -H "Content-Type: application/json" \
  -d '{"input": "busca o CPF 123.456.789-01"}'
# → {"command": "cpf", "args": "12345678901"}

# Step 2: Execute
curl "https://mbras-c2s.fly.dev/api/v1/work/modules/all?documento=12345678901"
```

### Pattern 2: Conversational Agent

Maintain conversation context for follow-ups:

```bash
curl -X POST https://mbras-c2s.fly.dev/api/v1/ai/interpret \
  -H "Content-Type: application/json" \
  -d '{
    "input": "enriquece ele",
    "messages": [
      {"role": "user", "content": "quem é o dono do telefone 11999887766?"},
      {"role": "assistant", "content": "João Silva, CPF 12345678901, Itaim Bibi"}
    ]
  }'
```

### Pattern 3: Content Generation from Customer Card

After looking up a customer, generate content using their data:

```
GET /api/v1/contributor/customer?phone=11999887766
  → customer response (JSON)

POST /api/v1/ai/generate
  → { template: "whatsapp_reply", context: <customer response> }
  → branded WhatsApp message
```

### Pattern 4: AI SDK Tool Calling (Vercel AI SDK)

Define tools that call the API endpoints — the AI decides when to invoke each:

```typescript
const tools = {
  lookupPerson: tool({
    description: "Look up person by CPF, phone, or email",
    inputSchema: z.object({
      cpf: z.string().optional(),
      phone: z.string().optional(),
      email: z.string().optional(),
    }),
    execute: async ({ cpf, phone, email }) => {
      const params = new URLSearchParams();
      if (cpf) params.set("cpf", cpf);
      if (phone) params.set("phone", phone);
      if (email) params.set("email", email);
      return fetch(`${BASE_URL}/api/v1/contributor/customer?${params}`);
    },
  }),
  lookupCompanies: tool({
    description: "Find companies where CPF is a partner (65M CNPJs)",
    inputSchema: z.object({ cpf: z.string() }),
    execute: async ({ cpf }) =>
      fetch(`${BASE_URL}/api/v1/company/cpf/${cpf}`),
  }),
  lookupProperties: tool({
    description: "Find real estate properties owned by CPF",
    inputSchema: z.object({ cpf: z.string() }),
    execute: async ({ cpf }) =>
      fetch(`${BASE_URL}/api/v1/property/cpf/${cpf}`),
  }),
};
```

### Pattern 5: Generic API Tool (Escape Hatch)

For endpoints not covered by specific tools:

```typescript
const callApi = tool({
  description: "Generic API call to MBRAS backend",
  inputSchema: z.object({
    method: z.enum(["GET", "POST", "PUT", "PATCH", "DELETE"]),
    path: z.string(),
    body: z.record(z.unknown()).optional(),
  }),
  execute: async ({ method, path, body }) => {
    return fetch(`${BASE_URL}${path}`, {
      method,
      body: body ? JSON.stringify(body) : undefined,
      headers: { "Content-Type": "application/json" },
    });
  },
});
```

---

## Error Handling

### HTTP Status Codes

| Status | Meaning | When |
|--------|---------|------|
| 200 | Success | Command interpreted or content generated |
| 400 | Bad Request | Empty input |
| 422 | Unprocessable | Missing required field (e.g., `template`, `context`) |
| 429 | Rate Limited | More than 10 req/s |
| 502 | Bad Gateway | OpenRouter unavailable, response parse error, or no content returned |
| 503 | Service Unavailable | `OPENROUTER_API_KEY` not configured |

### Error Response Format

```json
{
  "error": "AI service unavailable"
}
```

### Common Errors

| Error Message | Cause | Resolution |
|---------------|-------|------------|
| `"AI interpreter not configured"` | Missing `OPENROUTER_API_KEY` | Set env var on server |
| `"Input cannot be empty"` | Empty `input` field | Send non-empty input |
| `"AI service unavailable"` | OpenRouter request failed (network) | Retry after delay |
| `"AI error (HTTP 429)"` | OpenRouter rate limit | Back off, retry |
| `"AI error (HTTP 402)"` | OpenRouter credit exhausted | Add credits |
| `"AI response parse error"` | Model returned non-JSON (interpret) | Retry or use different model |
| `"AI returned no content"` | Model returned empty response | Retry |
| `"AI command parse error"` | Interpret response not valid JSON | Model hallucinated — retry |

---

## Rate Limits and Timeouts

### Server-Side

| Setting | Value |
|---------|-------|
| Rate limit | 10 requests/second (tower-governor) |
| Request size limit | 5 MB |
| Interpret timeout | 30 seconds (to OpenRouter) |
| Generate timeout | 60 seconds (to OpenRouter) |

### Recommended Client Settings

| Setting | Value | Why |
|---------|-------|-----|
| Client timeout | 35s (interpret), 65s (generate) | Slightly above server timeout |
| Retry strategy | 1 retry with 2s delay | Transient OpenRouter failures |
| Idempotency | Both endpoints are safe to retry | No side effects (read-only from CRM perspective) |

---

## Changelog

| Date | Change |
|------|--------|
| 2026-02-16 | Initial release: interpret (with conversation context), generate (5 templates), models endpoint |
| 2026-02-16 | Added MBRAS internal knowledge base to interpret system prompt |
| 2026-02-16 | Brand guardrails embedded in generate system prompt |
