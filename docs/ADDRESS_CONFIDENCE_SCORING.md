# Sistema de Confiança de Endereços

**Data:** 20/11/2025  
**Versão:** 1.0

## 🎯 Problema

A Work API retorna endereços associados ao CPF consultado, mas nem sempre são do titular:
- **Endereço do cônjuge**
- **Endereço dos pais**  
- **Endereço de outros familiares**
- **Endereços antigos**

**Exemplo:** Ao consultar João Silva, pode retornar o endereço onde sua mãe mora.

---

## ✅ Solução Implementada

### Sistema de Scoring de Confiança

Implementamos um sistema inteligente que:
1. **Analisa a posição** do endereço na resposta
2. **Detecta relacionamentos** (cônjuge, pais, etc)
3. **Atribui score de confiança** (0-100%)
4. **Armazena metadados** para auditoria

---

## 📊 Níveis de Confiança

### 🟢 Alta Confiança (90%)
**Critério:** Primeiro endereço retornado pela Work API, sem indicação de relacionamento

```rust
(0, None) => (0.90, "residential", true)
```

**Interpretação:** 
- É o endereço mais recente/relevante
- Muito provavelmente é onde a pessoa mora
- Marcado como `verified = true`
- Tipo: `residential`

### 🟡 Média Confiança (75%)
**Critério:** Endereços adicionais sem relacionamento explícito

```rust
_ => (0.75, "residential", false)
```

**Interpretação:**
- Pode ser endereço secundário
- Pode ser endereço antigo
- Requer validação adicional
- Tipo: `residential`

### 🟠 Baixa Confiança - Cônjuge (50%)
**Critério:** Endereço com relacionamento de cônjuge

```rust
if rel.contains("CÔNJUGE") || rel.contains("CONJUGE") => (0.50, "family_member", false)
```

**Interpretação:**
- Provavelmente mora com o cônjuge
- Pode ser endereço válido se morarem juntos
- Não é o endereço principal cadastrado no CPF
- Tipo: `family_member`

### 🔴 Muito Baixa Confiança - Pais (40%)
**Critério:** Endereço de pai ou mãe

```rust
if rel.contains("PAI") || rel.contains("MÃE") || rel.contains("MAE") => (0.40, "family_member", false)
```

**Interpretação:**
- Muito provavelmente não mora lá
- Pode ser endereço de referência
- Útil apenas para contexto familiar
- Tipo: `family_member`

### 🟣 Baixa Confiança - Outros Familiares (45%)
**Critério:** Outros relacionamentos familiares

```rust
(_, Some(_)) => (0.45, "family_member", false)
```

**Interpretação:**
- Endereço de parente
- Baixa probabilidade de ser o endereço atual
- Tipo: `family_member`

---

## 💾 Estrutura de Metadados

Cada relacionamento endereço-entity armazena:

```json
{
  "source": "work_api",
  "confidence_score": 0.90,
  "position_in_response": 0,
  "verified": true,
  "owner_name": "MARIA SILVA",  // Opcional
  "relationship": "CÔNJUGE"      // Opcional
}
```

### Campos:

- **source:** Origem dos dados (`work_api`)
- **confidence_score:** Score de 0.0 a 1.0
- **position_in_response:** Posição na lista (0 = primeiro)
- **verified:** Se foi verificado como pertencente à pessoa
- **owner_name:** Nome do titular (quando disponível)
- **relationship:** Tipo de relacionamento (quando disponível)

---

## 🔍 Queries Úteis

### Buscar apenas endereços de alta confiança

```sql
SELECT 
    e.name,
    e.national_id,
    a.neighborhood,
    a.city,
    a.formatted_address,
    ea.confidence_score,
    ea.address_type
FROM core.entities e
JOIN core.entity_addresses ea ON e.entity_id = ea.entity_id
JOIN core.addresses a ON ea.address_id = a.id
WHERE ea.confidence_score >= 0.75
AND e.is_enriched = true
ORDER BY ea.confidence_score DESC;
```

### Filtrar por bairros nobres COM alta confiança

```sql
SELECT 
    e.name,
    e.national_id,
    a.neighborhood,
    a.city,
    ea.confidence_score,
    ea.metadata->>'relationship' as relationship
FROM core.entities e
JOIN core.entity_addresses ea ON e.entity_id = ea.entity_id
JOIN core.addresses a ON ea.address_id = a.id
WHERE a.city ILIKE '%São Paulo%'
AND (
    a.neighborhood ILIKE '%Jardim Europa%' OR
    a.neighborhood ILIKE '%Vila Nova Conceição%' OR
    a.neighborhood ILIKE '%Cidade Jardim%'
)
AND ea.confidence_score >= 0.75  -- Apenas média/alta confiança
ORDER BY ea.confidence_score DESC, e.name;
```

### Ver todos os endereços com scores

```sql
SELECT 
    e.name,
    a.neighborhood,
    a.city,
    ea.address_type,
    ea.confidence_score,
    ea.verified,
    ea.metadata->>'relationship' as relationship,
    ea.metadata->>'owner_name' as owner_name
FROM core.entities e
JOIN core.entity_addresses ea ON e.entity_id = ea.entity_id
JOIN core.addresses a ON ea.address_id = a.id
WHERE e.national_id = '12345678901'
ORDER BY ea.confidence_score DESC;
```

### Estatísticas de confiança

```sql
SELECT 
    CASE 
        WHEN ea.confidence_score >= 0.90 THEN 'Alta (90%+)'
        WHEN ea.confidence_score >= 0.75 THEN 'Média (75-89%)'
        WHEN ea.confidence_score >= 0.50 THEN 'Baixa (50-74%)'
        ELSE 'Muito Baixa (<50%)'
    END as nivel_confianca,
    ea.address_type,
    COUNT(*) as quantidade,
    ROUND(AVG(ea.confidence_score::numeric) * 100, 1) as score_medio
FROM core.entity_addresses ea
WHERE ea.created_at > NOW() - INTERVAL '7 days'
GROUP BY 
    CASE 
        WHEN ea.confidence_score >= 0.90 THEN 'Alta (90%+)'
        WHEN ea.confidence_score >= 0.75 THEN 'Média (75-89%)'
        WHEN ea.confidence_score >= 0.50 THEN 'Baixa (50-74%)'
        ELSE 'Muito Baixa (<50%)'
    END,
    ea.address_type
ORDER BY score_medio DESC;
```

---

## 📈 Exemplo de Uso

### Cenário: João Silva

**Work API retorna:**
1. Rua A, 100 - Jardim Europa (sem relacionamento) → **90% confiança**
2. Rua B, 200 - Moema (cônjuge: Maria Silva) → **50% confiança**  
3. Rua C, 300 - Pinheiros (mãe: Ana Silva) → **40% confiança**

**Banco de dados armazena:**

| Endereço | Tipo | Confiança | Verificado | Relacionamento |
|----------|------|-----------|------------|----------------|
| Rua A, 100 - Jardim Europa | residential | 90% | ✓ | - |
| Rua B, 200 - Moema | family_member | 50% | ✗ | CÔNJUGE |
| Rua C, 300 - Pinheiros | family_member | 40% | ✗ | MÃE |

**Para análise de bairros nobres:**
- ✅ Usar Rua A (90%) - João provavelmente mora no Jardim Europa
- ⚠️  Considerar Rua B (50%) - Pode morar com esposa em Moema
- ❌ Ignorar Rua C (40%) - Endereço da mãe, não mora lá

---

## 🚀 Benefícios

1. **Precisão:** Identifica qual endereço realmente pertence à pessoa
2. **Transparência:** Score visível para análise
3. **Flexibilidade:** Pode filtrar por nível de confiança
4. **Auditoria:** Metadados completos para rastreamento
5. **Inteligência:** Detecta relacionamentos automaticamente

---

## 🔄 Fluxo de Processamento

```
1. Work API retorna endereços
          ↓
2. Sistema analisa posição e relacionamento
          ↓
3. Atribui score de confiança (40-90%)
          ↓
4. Define tipo (residential / family_member)
          ↓
5. Salva com metadados completos
          ↓
6. Log detalhado: "✓ Linked address ... (confidence: 90%)"
```

---

## 📝 Logs Exemplo

```log
✓ Linked address 550e8400-... to entity 123e4567-... (type: residential, primary: true, confidence: 90%)
✓ Linked address 6ba7b810-... to entity 123e4567-... (type: family_member, primary: false, confidence: 50%)
✓ Linked address 6ba7b811-... to entity 123e4567-... (type: family_member, primary: false, confidence: 40%)
```

---

## ⚙️ Configuração

O scoring é configurado diretamente no código em `src/db_storage.rs`:

```rust
let (confidence_score, address_type_str, verified) = match (idx, relationship) {
    (0, None) => (0.90, "residential", true),
    (_, Some(rel)) if rel.contains("CÔNJUGE") => (0.50, "family_member", false),
    (_, Some(rel)) if rel.contains("PAI") || rel.contains("MÃE") => (0.40, "family_member", false),
    (_, Some(_)) => (0.45, "family_member", false),
    _ => (0.75, "residential", false),
};
```

**Para ajustar os scores:** Modifique os valores acima e recompile.

---

## 🧪 Como Testar

### 1. Enriquecer um lead
```bash
curl -X POST https://mbras-c2s.fly.dev/api/v1/c2s/enrich/LEAD_ID
```

### 2. Verificar scores no banco
```bash
psql $DB_URL -c "
SELECT 
    e.name,
    a.neighborhood,
    ea.confidence_score,
    ea.address_type,
    ea.metadata->>'relationship' as rel
FROM core.entities e
JOIN core.entity_addresses ea ON e.entity_id = ea.entity_id
JOIN core.addresses a ON ea.address_id = a.id
WHERE e.metadata->>'c2s_lead_id' = 'LEAD_ID'
ORDER BY ea.confidence_score DESC
"
```

---

**Status:** ✅ Implementado e testado  
**Compilação:** ✅ Sem erros  
**Próximo:** Deploy para produção
