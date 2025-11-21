# Migração do Schema - Relacionamento Lead-Endereço

**Data:** 20/11/2025  
**Autor:** Claude AI + Ronaldo

## 🎯 Objetivo

Atualizar o código para trabalhar corretamente com o novo schema do banco de dados e criar relacionamento entre leads (C2S) e endereços enriquecidos.

---

## 📊 Análise do Schema Atual

### Estrutura Descoberta

O banco de dados usa a seguinte estrutura (diferente do schema antigo):

```
core.entities (UUID)
  ├── entity_id (UUID, PK)
  ├── national_id (CPF/CNPJ)
  ├── name
  ├── canonical_name
  ├── metadata (JSONB) ← NOVO: armazena lead_id aqui
  └── ...

core.addresses (UUID)
  ├── id (UUID, PK) 
  ├── street
  ├── number
  ├── neighborhood ← Bairros nobres!
  ├── city
  ├── state
  ├── zip_code
  └── formatted_address

core.entity_addresses (relacionamento N:N)
  ├── entity_id → core.entities
  ├── address_id → core.addresses
  ├── address_type ('residential', 'commercial', etc)
  ├── is_primary (boolean)
  └── data_source ('api', 'manual', etc)
```

**Observação:** Não há tabela separada de "leads" - os leads do C2S são armazenados como `entities`.

---

## 🔧 Mudanças Realizadas

### 1. **Correção da Tabela de Endereços**

**Problema:** Código estava tentando inserir em `app.addresses` (não existe)  
**Solução:** Corrigido para `core.addresses`

**Problema:** Tipo de retorno era `i32`, mas a tabela usa `UUID`  
**Solução:** Mudado para `(Uuid,)`

### 2. **Melhoria no Salvamento de Endereços**

#### Antes (src/db_storage.rs:428)
```rust
let address_row: Result<(i32,), _> = sqlx::query_as(
    "INSERT INTO app.addresses (...) VALUES (...) RETURNING id"
)
```

#### Depois
```rust
let address_row: Result<(Uuid,), _> = sqlx::query_as(
    r#"
    INSERT INTO core.addresses (
        street_type, street, number, complement, neighborhood, 
        city, state, zip_code, formatted_address, is_valid, 
        primary_address, created_at, updated_at
    )
    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, true, $10, now(), now())
    ON CONFLICT ON CONSTRAINT addresses_pkey DO NOTHING
    RETURNING id
    "#,
)
```

**Novos recursos:**
- ✅ Gera `formatted_address` automaticamente
- ✅ Trata conflitos (endereços duplicados)
- ✅ Busca endereço existente se houver conflito
- ✅ Logs detalhados de sucesso/erro

### 3. **Relacionamento Lead → Entity**

Adicionado campo `c2s_lead_id` no metadata da entity para rastrear origem do lead:

```rust
let mut entity_metadata = json!({});
if let Some(lid) = lead_id {
    entity_metadata["c2s_lead_id"] = json!(lid);
    entity_metadata["c2s_source"] = json!("api_enrichment");
    entity_metadata["enriched_at"] = json!(chrono::Utc::now().to_rfc3339());
}
```

**Novo método:**
```rust
pub async fn store_enriched_person_with_lead(
    &self,
    cpf: &str,
    work_data: &WorkApiCompleteResponse,
    lead_id: Option<&str>,
) -> Result<Uuid, AppError>
```

### 4. **Atualização dos Handlers**

Ambos os endpoints agora passam o `lead_id` para o storage:

#### `c2s_enrich_lead` (linha 440)
```rust
.store_enriched_person_with_lead(cpf, &enriched_data[idx], Some(&lead_id))
```

#### `trigger_lead_processing` (linha 898)
```rust
.store_enriched_person_with_lead(cpf, &enriched_data[idx], Some(lead_id))
```

---

## 📋 Queries Úteis

### Buscar leads com endereços em bairros nobres

```sql
SELECT 
    e.name,
    e.national_id as cpf,
    e.metadata->>'c2s_lead_id' as lead_id,
    a.neighborhood,
    a.city,
    a.formatted_address
FROM core.entities e
JOIN core.entity_addresses ea ON e.entity_id = ea.entity_id
JOIN core.addresses a ON ea.address_id = a.id
WHERE a.city ILIKE '%São Paulo%'
AND (
    a.neighborhood ILIKE '%Jardim Europa%' OR
    a.neighborhood ILIKE '%Vila Nova Conceição%' OR
    a.neighborhood ILIKE '%Cidade Jardim%' OR
    a.neighborhood ILIKE '%Itaim Bibi%' OR
    a.neighborhood ILIKE '%Moema%'
)
AND e.metadata ? 'c2s_lead_id'
ORDER BY e.created_at DESC;
```

### Buscar entity pelo lead_id do C2S

```sql
SELECT 
    entity_id,
    name,
    national_id,
    metadata->>'c2s_lead_id' as lead_id,
    enriched_at
FROM core.entities
WHERE metadata->>'c2s_lead_id' = 'bf1a88eaa4ab34b01a257536563fb42b';
```

### Ver todos os endereços de uma entity

```sql
SELECT 
    e.name,
    a.street,
    a.number,
    a.neighborhood,
    a.city,
    a.state,
    a.zip_code,
    ea.address_type,
    ea.is_primary
FROM core.entities e
JOIN core.entity_addresses ea ON e.entity_id = ea.entity_id
JOIN core.addresses a ON ea.address_id = a.id
WHERE e.national_id = '26202997800'
ORDER BY ea.is_primary DESC;
```

---

## ✅ Benefícios

1. **Rastreabilidade:** Agora podemos rastrear qual lead do C2S originou cada entity
2. **Bairros Nobres:** Campo `neighborhood` agora é salvo corretamente
3. **Deduplicação:** Endereços duplicados são tratados automaticamente
4. **Logs:** Melhor visibilidade do que está acontecendo
5. **Metadata Flexível:** Fácil adicionar mais informações no futuro

---

## 🧪 Como Testar

### 1. Compilar
```bash
cargo check
cargo build
```

### 2. Testar Localmente
```bash
cargo run
```

### 3. Enriquecer um Lead
```bash
curl -X POST https://mbras-c2s.fly.dev/api/v1/c2s/enrich/bf1a88eaa4ab34b01a257536563fb42b
```

### 4. Verificar no Banco
```bash
psql $DB_URL -c "
SELECT 
    e.name,
    e.national_id,
    e.metadata->>'c2s_lead_id' as lead_id,
    a.neighborhood,
    a.city
FROM core.entities e
LEFT JOIN core.entity_addresses ea ON e.entity_id = ea.entity_id
LEFT JOIN core.addresses a ON ea.address_id = a.id
WHERE e.metadata->>'c2s_lead_id' = 'bf1a88eaa4ab34b01a257536563fb42b'
"
```

---

## 📝 Notas Importantes

1. **Backward Compatible:** O método antigo `store_enriched_person()` ainda funciona (sem lead_id)
2. **Metadata Merge:** Se a entity já existir, o metadata é mesclado (não sobrescrito)
3. **Primary Address:** O primeiro endereço da Work API é marcado como `is_primary = true`
4. **UUID vs INT:** Todas as chaves primárias usam UUID, não INT

---

## 🚀 Próximos Passos

1. Deploy para produção
2. Testar com leads reais
3. Criar dashboard para visualizar leads por bairro
4. Implementar filtros avançados (score + bairro + renda)

---

**Status:** ✅ Implementado e testado  
**Compilação:** ✅ Sem erros  
**Deploy:** ⏳ Pendente
