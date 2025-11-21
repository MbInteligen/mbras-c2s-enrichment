# Plano Completo: Webhooks C2S Diretos + Redis para Multi-Instance

## Objetivo
1. Receber webhooks diretamente do C2S (eliminar Make.com)
2. Implementar Redis para deduplicação atômica (permitir múltiplas instâncias)

---

## PARTE 1: Redis para Deduplicação Multi-Instance

### 1.1 Adicionar Dependências Redis
**Arquivo**: `Cargo.toml`
```toml
redis = { version = "0.24", features = ["tokio-comp", "connection-manager"] }
```

### 1.2 Configurar Redis no Fly.io
**Comando**: 
```bash
fly redis create mbras-c2s-redis --region gru
fly redis attach mbras-c2s-redis
```
Isso cria variável `REDIS_URL` automaticamente

**Arquivo**: `src/config.rs`
- Adicionar campo `redis_url: String`
- Carregar de `REDIS_URL` env var

### 1.3 Substituir Cache In-Memory por Redis
**Arquivo**: `src/main.rs`
- Remover `processing_leads_cache` (moka)
- Criar `RedisConnectionManager`
- Passar para `AppState`

**Arquivo**: `src/handlers.rs` - `trigger_lead_processing()`
```rust
// Deduplicação atômica com Redis
let lock_key = format!("lead:processing:{}", lead_id);
let acquired = redis::cmd("SET")
    .arg(&lock_key)
    .arg(now)
    .arg("NX")  // Only set if not exists
    .arg("EX")  // Expire
    .arg(300)   // 5 minutes
    .query_async(&mut redis_conn)
    .await?;

if !acquired {
    return DUPLICATE_REQUEST_BLOCKED;
}
```

### 1.4 Atualizar Dockerfile
**Arquivo**: `Dockerfile`
- Não precisa mudar (Fly Redis é externo via rede)

---

## PARTE 2: Webhook Direto do C2S

### 2.1 Criar Endpoint de Webhook
**Arquivo**: `src/handlers.rs`
```rust
pub async fn c2s_webhook_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<C2SWebhookPayload>,
) -> Result<Json<Value>, AppError>
```

**Validações**:
- Verificar HMAC signature no header `X-C2S-Signature`
- Verificar timestamp (rejeitar se > 5 minutos)
- Retornar 200 rapidamente

### 2.2 Estruturas de Dados
**Arquivo**: `src/models.rs`
```rust
#[derive(Deserialize)]
pub struct C2SWebhookPayload {
    pub lead_id: String,
    pub action: String,  // "on_create_lead", etc.
    pub timestamp: i64,
}
```

### 2.3 Validação HMAC
**Arquivo**: `src/handlers.rs` ou `src/security.rs` (novo)
```rust
fn validate_c2s_signature(
    payload: &str,
    signature: &str,
    secret: &str
) -> bool {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(payload.as_bytes());
    let expected = hex::encode(mac.finalize().into_bytes());
    
    expected == signature
}
```

**Adicionar ao Cargo.toml**:
```toml
hmac = "0.12"
sha2 = "0.10"
hex = "0.4"
```

### 2.4 Configuração
**Arquivo**: `src/config.rs`
```rust
pub struct Config {
    // ... existing
    pub c2s_webhook_secret: String,
    pub c2s_webhook_timeout_secs: u64,
}
```

**Env vars**:
- `C2S_WEBHOOK_SECRET` - shared secret para HMAC
- `C2S_WEBHOOK_TIMEOUT_SECS` - default 300

### 2.5 Tabela de Audit (Webhook Events)
**Arquivo**: `migrations/003_webhook_events.sql` (novo)
```sql
CREATE TABLE IF NOT EXISTS webhook_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    lead_id TEXT NOT NULL,
    hook_action TEXT NOT NULL,
    received_at TIMESTAMPTZ DEFAULT NOW(),
    processed_at TIMESTAMPTZ,
    status TEXT NOT NULL, -- 'processing', 'success', 'failed'
    error_message TEXT,
    retry_count INT DEFAULT 0,
    raw_payload JSONB
);

CREATE INDEX idx_webhook_events_lead_id ON webhook_events(lead_id);
CREATE INDEX idx_webhook_events_status ON webhook_events(status);
CREATE INDEX idx_webhook_events_received_at ON webhook_events(received_at DESC);
```

### 2.6 Deduplicação com DB + Redis
**Fluxo em `c2s_webhook_handler`**:
1. **Redis lock** (atômico, multi-instance safe)
   ```rust
   SET lead:processing:{lead_id} {timestamp} NX EX 300
   ```
2. **DB insert** (audit + deduplicação persistente)
   ```sql
   INSERT INTO webhook_events (lead_id, hook_action, status, raw_payload)
   VALUES ($1, $2, 'processing', $3)
   ON CONFLICT (lead_id, hook_action) 
   WHERE status IN ('processing', 'success')
   DO NOTHING RETURNING id;
   ```
   - Se retornar NULL → duplicata, retornar 200
3. **Processar lead** (reusa `process_lead_internal`)
4. **Atualizar DB**:
   ```sql
   UPDATE webhook_events 
   SET status='success', processed_at=NOW()
   WHERE id = $1;
   ```
5. **Release Redis lock** (automático via EX, mas pode deletar manualmente)

### 2.7 Refatorar Lógica de Processamento
**Arquivo**: `src/handlers.rs`
```rust
// Função helper reutilizável
async fn process_lead_internal(
    state: &AppState,
    lead_id: &str,
) -> Result<ProcessedLeadResult, AppError> {
    // Move toda lógica de:
    // - fetch_lead from C2S
    // - diretrix lookup
    // - work_api enrichment
    // - format message
    // - send_message to C2S
    // - store in DB
}

// Usar em:
pub async fn c2s_webhook_handler(...) {
    // ... validação, dedup ...
    let result = process_lead_internal(&state, &lead_id).await?;
    // ... update DB ...
}

pub async fn trigger_lead_processing(...) {
    // ... dedup ...
    let result = process_lead_internal(&state, lead_id).await?;
}
```

### 2.8 Registrar Webhook no C2S
**Arquivo**: `docs/scripts/register_c2s_webhook.sh` (novo)
```bash
#!/bin/bash
C2S_TOKEN="${C2S_TOKEN}"
WEBHOOK_URL="https://mbras-c2s.fly.dev/api/v1/webhook/leads"
WEBHOOK_SECRET="${C2S_WEBHOOK_SECRET}"

curl -X POST https://api.contact2sale.com/integration/leads/subscribe \
  -H "Authorization: Bearer $C2S_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "url": "'"$WEBHOOK_URL"'",
    "action": "on_create_lead",
    "secret": "'"$WEBHOOK_SECRET"'"
  }'
```

**Arquivo**: `docs/scripts/unregister_c2s_webhook.sh` (novo)
```bash
curl -X DELETE https://api.contact2sale.com/integration/leads/subscribe \
  -H "Authorization: Bearer $C2S_TOKEN"
```

---

## PARTE 3: Testes e Validação

### 3.1 Script de Teste de Webhook
**Arquivo**: `docs/scripts/test_webhook_signature.sh`
```bash
#!/bin/bash
# Simula webhook do C2S com HMAC válido
LEAD_ID="${1:-test-lead-123}"
SECRET="${C2S_WEBHOOK_SECRET}"
TIMESTAMP=$(date +%s)

PAYLOAD='{"lead_id":"'$LEAD_ID'","action":"on_create_lead","timestamp":'$TIMESTAMP'}'
SIGNATURE=$(echo -n "$PAYLOAD" | openssl dgst -sha256 -hmac "$SECRET" | awk '{print $2}')

curl -X POST https://mbras-c2s.fly.dev/api/v1/webhook/leads \
  -H "Content-Type: application/json" \
  -H "X-C2S-Signature: $SIGNATURE" \
  -d "$PAYLOAD"
```

### 3.2 Teste de Múltiplas Instâncias
**Arquivo**: `fly.toml`
```toml
[[vm]]
  memory = '256mb'
  cpu_kind = 'shared'
  cpus = 1

[http_service]
  # ...
  min_machines_running = 2  # Testar com 2 instâncias
```

**Teste**:
```bash
fly scale count 2
./docs/scripts/test_concurrent_requests.sh <lead_id>
# Verificar logs: apenas 1 instância deve processar
```

### 3.3 Testes Unitários
**Arquivo**: `tests/webhook_tests.rs` (novo)
- Teste de validação HMAC (válido/inválido)
- Teste de timestamp expirado
- Teste de deduplicação

---

## PARTE 4: Deploy e Migração

### 4.1 Setup Redis no Fly.io
```bash
fly redis create mbras-c2s-redis --region gru
fly redis attach mbras-c2s-redis
fly secrets set C2S_WEBHOOK_SECRET="<gerar-secret-forte>"
```

### 4.2 Deploy Gradual
**Fase 1**: Deploy com webhook endpoint (sem registrar no C2S)
```bash
git add -A
git commit -m "feat: add C2S webhook endpoint + Redis deduplication"
git push
fly deploy
```

**Fase 2**: Testar localmente/manualmente
```bash
./docs/scripts/test_webhook_signature.sh test-lead-001
# Verificar logs
fly logs
```

**Fase 3**: Registrar webhook no C2S (paralelo ao Make)
```bash
./docs/scripts/register_c2s_webhook.sh
```

**Fase 4**: Monitorar 24-48h
```bash
fly logs --filter webhook
# Verificar métricas no DB
psql $DATABASE_URL -c "SELECT status, COUNT(*) FROM webhook_events GROUP BY status;"
```

**Fase 5**: Desativar Make.com
- Pausar scenario no Make
- Monitorar por mais 24h
- Deletar scenario

**Fase 6**: (Opcional) Escalar para múltiplas instâncias
```bash
fly scale count 2
fly scale memory 512  # Se necessário
```

---

## PARTE 5: Monitoring e Observabilidade

### 5.1 Métricas no Banco
**Queries úteis**:
```sql
-- Webhooks por status (últimas 24h)
SELECT status, COUNT(*), 
       AVG(EXTRACT(EPOCH FROM (processed_at - received_at))) as avg_duration_secs
FROM webhook_events 
WHERE received_at > NOW() - INTERVAL '24 hours'
GROUP BY status;

-- Leads com múltiplas tentativas
SELECT lead_id, COUNT(*), MAX(retry_count)
FROM webhook_events
GROUP BY lead_id
HAVING COUNT(*) > 1
ORDER BY COUNT(*) DESC;

-- Taxa de sucesso
SELECT 
  ROUND(100.0 * SUM(CASE WHEN status='success' THEN 1 ELSE 0 END) / COUNT(*), 2) as success_rate
FROM webhook_events
WHERE received_at > NOW() - INTERVAL '24 hours';
```

### 5.2 Alertas
**Implementar**:
- Se `retry_count > 3` → alerta
- Se `status='failed'` e error_message contém "C2S" → alerta de integração
- Se taxa de duplicatas > 20% → investigar

---

## Arquivos a Criar/Modificar

### Criar
- [ ] `migrations/003_webhook_events.sql`
- [ ] `src/security.rs` (validação HMAC)
- [ ] `docs/scripts/register_c2s_webhook.sh`
- [ ] `docs/scripts/unregister_c2s_webhook.sh`
- [ ] `docs/scripts/test_webhook_signature.sh`
- [ ] `tests/webhook_tests.rs`
- [ ] `docs/REDIS_SETUP.md`

### Modificar
- [ ] `Cargo.toml` (adicionar redis, hmac, sha2, hex)
- [ ] `src/config.rs` (redis_url, webhook_secret, webhook_timeout)
- [ ] `src/main.rs` (setup Redis connection manager)
- [ ] `src/handlers.rs` (novo endpoint, refatorar lógica, Redis dedup)
- [ ] `src/models.rs` (C2SWebhookPayload)
- [ ] `fly.toml` (opcional: min_machines_running)

---

## Benefícios da Solução Completa

✅ **Elimina Make.com** - reduz custo e latência  
✅ **Multi-instance ready** - Redis permite escalar horizontalmente  
✅ **Deduplicação robusta** - Redis (atômico) + DB (persistente)  
✅ **Audit trail completo** - tabela webhook_events  
✅ **Segurança** - validação HMAC, timeout de requisições  
✅ **Observabilidade** - logs detalhados, métricas no DB  
✅ **Testável** - scripts de simulação, testes unitários  

---

## Estimativa de Tempo

1. Redis setup + código: **2-3 horas**
2. Webhook endpoint + validação: **2-3 horas**
3. Refatoração + testes: **2-3 horas**
4. Deploy + monitoramento: **1-2 horas**
5. Migração gradual: **24-48 horas** (monitoramento)

**Total**: ~1-2 dias de desenvolvimento + 2 dias de validação

---

## Notas Importantes

1. **C2S API Documentation**: Confirmar formato exato do webhook payload e header de assinatura
2. **Redis no Fly.io**: Upstash Redis é gratuito até 10k comandos/dia
3. **Backward compatibility**: Manter endpoint `/api/v1/leads/process` ativo durante migração
4. **Secrets rotation**: Implementar rotação do `C2S_WEBHOOK_SECRET` periodicamente
5. **Rate limiting**: Considerar adicionar rate limit por lead_id para evitar abuse

---

Plano criado em: 2025-01-14
Status: Pronto para implementação
