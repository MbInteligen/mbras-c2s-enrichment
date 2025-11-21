# on_close_lead Webhook - Future Implementation Plan

**Purpose**: Notify realtor manager when leads are closed  
**Status**: ✅ Subscribed, ⏳ Implementation Pending  
**Subscription Date**: 2025-11-21  
**Priority**: Medium (planned for near future)

---

## Current Status

### Webhook Subscription ✅
- **Subscribed**: 2025-11-21T04:50:00Z
- **Endpoint**: https://mbras-c2s.fly.dev/api/v1/webhooks/c2s
- **Hook Action**: `on_close_lead`
- **Response**: `{"success":true,"message":"Subscribed successfully"}`

### Current Behavior ⚠️
- Webhook is **received** and stored in `webhook_events` table
- Background enrichment job **runs** but does nothing special for closed leads
- No manager notification sent (feature not yet implemented)

---

## Future Implementation Plan

### Phase 1: Basic Manager Notification (MVP)

**Goal**: Send notification to realtor manager when lead closes

#### 1.1 Database Schema

Add manager notification tracking table:

```sql
-- Manager notification log
CREATE TABLE IF NOT EXISTS manager_notifications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    lead_id TEXT NOT NULL,
    webhook_event_id UUID REFERENCES webhook_events(id),
    manager_email TEXT NOT NULL,
    manager_name TEXT,
    realtor_name TEXT,
    lead_status TEXT,  -- won, lost, cancelled
    close_reason TEXT,
    notification_sent_at TIMESTAMPTZ,
    notification_status TEXT DEFAULT 'pending',  -- pending, sent, failed
    notification_channel TEXT,  -- email, sms, whatsapp, c2s_message
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT fk_webhook_event FOREIGN KEY (webhook_event_id) 
        REFERENCES webhook_events(id) ON DELETE SET NULL
);

CREATE INDEX idx_manager_notifications_lead_id ON manager_notifications(lead_id);
CREATE INDEX idx_manager_notifications_status ON manager_notifications(notification_status);
CREATE INDEX idx_manager_notifications_sent_at ON manager_notifications(notification_sent_at);
```

#### 1.2 Manager Configuration

Store manager assignments (which realtor belongs to which manager):

```sql
-- Realtor-Manager mapping
CREATE TABLE IF NOT EXISTS realtor_managers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    realtor_id TEXT NOT NULL,  -- C2S user ID
    realtor_name TEXT NOT NULL,
    realtor_email TEXT,
    manager_id TEXT NOT NULL,
    manager_name TEXT NOT NULL,
    manager_email TEXT NOT NULL,
    manager_phone TEXT,
    active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT uq_realtor_id UNIQUE (realtor_id)
);

CREATE INDEX idx_realtor_managers_realtor_id ON realtor_managers(realtor_id);
CREATE INDEX idx_realtor_managers_manager_id ON realtor_managers(manager_id);
CREATE INDEX idx_realtor_managers_active ON realtor_managers(active) WHERE active = true;
```

#### 1.3 Code Changes

**File**: `src/handlers.rs`

Add new handler for closed leads:

```rust
async fn handle_closed_lead(
    lead_id: &str,
    lead_data: &serde_json::Value,
    config: &Config,
) -> Result<(), AppError> {
    tracing::info!("Processing closed lead: {}", lead_id);
    
    // 1. Extract lead details
    let lead_status = lead_data
        .get("attributes")
        .and_then(|a| a.get("lead_status"))
        .and_then(|s| s.get("alias"))
        .and_then(|a| a.as_str())
        .unwrap_or("closed");
    
    let realtor_id = lead_data
        .get("attributes")
        .and_then(|a| a.get("user"))
        .and_then(|u| u.get("id"))
        .and_then(|i| i.as_str())
        .unwrap_or("");
    
    let customer_name = lead_data
        .get("attributes")
        .and_then(|a| a.get("customer"))
        .and_then(|c| c.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("Unknown");
    
    // 2. Find manager for this realtor
    let manager = get_manager_for_realtor(realtor_id).await?;
    
    // 3. Send notification to manager
    send_manager_notification(
        &manager,
        lead_id,
        customer_name,
        lead_status,
        lead_data,
    ).await?;
    
    Ok(())
}
```

**File**: `src/services.rs`

Add manager notification service:

```rust
pub struct ManagerNotificationService {
    config: Config,
}

impl ManagerNotificationService {
    pub async fn send_notification(
        &self,
        manager_email: &str,
        lead_id: &str,
        customer_name: &str,
        lead_status: &str,
        lead_data: &serde_json::Value,
    ) -> Result<(), AppError> {
        // Format notification message
        let message = format!(
            "🔔 Lead Fechado\n\n\
            Cliente: {}\n\
            Status: {}\n\
            Lead ID: {}\n\n\
            Detalhes disponíveis no C2S.",
            customer_name,
            lead_status,
            lead_id
        );
        
        // TODO: Choose notification channel
        // - Email (via SendGrid/AWS SES)
        // - SMS (via Twilio)
        // - WhatsApp (via Twilio/MessageBird)
        // - C2S internal message
        
        // For MVP: Send via email
        self.send_email_notification(manager_email, &message).await?;
        
        Ok(())
    }
    
    async fn send_email_notification(
        &self,
        to: &str,
        message: &str,
    ) -> Result<(), AppError> {
        // TODO: Implement email sending
        // Use SendGrid, AWS SES, or similar
        tracing::info!("Would send email to {} with message: {}", to, message);
        Ok(())
    }
}
```

#### 1.4 Webhook Handler Update

**File**: `src/handlers.rs` - Update `process_webhook_event()`

```rust
pub async fn process_webhook_event(
    event: &WebhookEvent,
    state: &AppState,
) -> Result<(), AppError> {
    let lead_data: serde_json::Value = event.payload_raw.clone();
    let hook_action = event.hook_action.as_deref().unwrap_or("");
    
    match hook_action {
        "on_create_lead" | "on_update_lead" => {
            // Existing enrichment logic
            enrich_and_send_to_c2s(&event.lead_id, &lead_data, &state.config).await?;
        }
        "on_close_lead" => {
            // NEW: Manager notification logic
            handle_closed_lead(&event.lead_id, &lead_data, &state.config).await?;
        }
        _ => {
            tracing::warn!("Unknown hook_action: {}", hook_action);
        }
    }
    
    Ok(())
}
```

---

### Phase 2: Enhanced Notifications (Future)

#### 2.1 Rich Notification Templates

**Email Template**:
```html
<!DOCTYPE html>
<html>
<head>
    <style>
        body { font-family: Arial, sans-serif; }
        .header { background: #4CAF50; color: white; padding: 20px; }
        .content { padding: 20px; }
        .status-won { color: #4CAF50; }
        .status-lost { color: #f44336; }
    </style>
</head>
<body>
    <div class="header">
        <h2>🎯 Lead Fechado - MBRAS</h2>
    </div>
    <div class="content">
        <h3>Informações do Lead</h3>
        <p><strong>Cliente:</strong> {{customer_name}}</p>
        <p><strong>Telefone:</strong> {{customer_phone}}</p>
        <p><strong>Email:</strong> {{customer_email}}</p>
        <p><strong>Status:</strong> <span class="status-{{status}}">{{status_text}}</span></p>
        <p><strong>Corretor:</strong> {{realtor_name}}</p>
        <p><strong>Imóvel:</strong> {{property_description}}</p>
        <p><strong>Valor:</strong> {{property_price}}</p>
        
        <h3>Enriquecimento CPF</h3>
        <p><strong>CPF:</strong> {{cpf}}</p>
        <p><strong>Score:</strong> {{credit_score}}</p>
        <p><strong>Renda Estimada:</strong> {{estimated_income}}</p>
        
        <a href="https://app.contact2sale.com/leads/{{lead_id}}" 
           style="background: #4CAF50; color: white; padding: 10px 20px; text-decoration: none; border-radius: 5px;">
            Ver Lead no C2S
        </a>
    </div>
</body>
</html>
```

#### 2.2 Multi-Channel Support

**Configuration** (`config.rs`):
```rust
pub struct NotificationConfig {
    pub channels: Vec<NotificationChannel>,
    pub email_provider: EmailProvider,
    pub sms_provider: Option<SmsProvider>,
    pub whatsapp_provider: Option<WhatsAppProvider>,
}

pub enum NotificationChannel {
    Email,
    Sms,
    WhatsApp,
    C2sMessage,
}
```

**Channel Priority**:
1. **Email** (primary, always sent)
2. **SMS** (optional, for urgent notifications)
3. **WhatsApp** (optional, for rich media)
4. **C2S Message** (fallback, internal notification)

#### 2.3 Manager Dashboard (Future Web UI)

**Endpoint**: `GET /api/v1/manager/dashboard`

**Features**:
- View all closed leads (today, this week, this month)
- Filter by status (won, lost, cancelled)
- Filter by realtor
- Export to CSV/Excel
- Performance metrics (conversion rate, avg deal value)

---

### Phase 3: Analytics & Reporting (Future)

#### 3.1 Lead Closure Analytics

```sql
-- Daily closure statistics
CREATE MATERIALIZED VIEW daily_closure_stats AS
SELECT 
    DATE(webhook_events.received_at) as date,
    COUNT(*) as total_closures,
    COUNT(*) FILTER (WHERE payload_raw->'attributes'->'lead_status'->>'alias' = 'won') as won,
    COUNT(*) FILTER (WHERE payload_raw->'attributes'->'lead_status'->>'alias' = 'lost') as lost,
    COUNT(*) FILTER (WHERE payload_raw->'attributes'->'lead_status'->>'alias' = 'cancelled') as cancelled,
    ROUND(100.0 * COUNT(*) FILTER (WHERE payload_raw->'attributes'->'lead_status'->>'alias' = 'won') / COUNT(*), 2) as win_rate
FROM webhook_events
WHERE hook_action = 'on_close_lead'
GROUP BY DATE(webhook_events.received_at)
ORDER BY date DESC;

-- Refresh materialized view daily
REFRESH MATERIALIZED VIEW daily_closure_stats;
```

#### 3.2 Manager Reports

**Weekly Email Report**:
- Total leads closed this week
- Win rate vs last week
- Top performing realtors
- Average time to close
- Revenue generated (if available)

**Monthly Summary**:
- Monthly closure trends
- Realtor performance rankings
- Lead source analysis
- Customer demographics (from enriched data)

---

## Implementation Roadmap

### Immediate (Week 1-2)
- [x] Subscribe to `on_close_lead` webhook
- [ ] Create database migrations (manager_notifications, realtor_managers)
- [ ] Implement basic manager lookup logic
- [ ] Add closed lead detection in webhook handler

### Short-term (Week 3-4)
- [ ] Implement email notification service (SendGrid/AWS SES)
- [ ] Create simple email templates
- [ ] Add manager configuration UI/API
- [ ] Test with real closed leads

### Medium-term (Month 2)
- [ ] Add SMS notification support (Twilio)
- [ ] Enhance email templates (rich HTML)
- [ ] Create manager dashboard (web UI)
- [ ] Add notification preferences per manager

### Long-term (Month 3+)
- [ ] WhatsApp integration
- [ ] Advanced analytics dashboard
- [ ] Automated weekly/monthly reports
- [ ] Mobile app notifications (push)
- [ ] AI-powered insights (win/loss prediction)

---

## Configuration Requirements

### Environment Variables (Future)

```bash
# Email Provider (SendGrid)
SENDGRID_API_KEY=<api_key>
SENDGRID_FROM_EMAIL=noreply@mbras.com.br
SENDGRID_FROM_NAME="MBRAS Notificações"

# SMS Provider (Twilio) - Optional
TWILIO_ACCOUNT_SID=<sid>
TWILIO_AUTH_TOKEN=<token>
TWILIO_FROM_PHONE=+5511999999999

# WhatsApp Provider - Optional
WHATSAPP_API_KEY=<api_key>
WHATSAPP_PHONE_ID=<phone_id>

# Notification Settings
ENABLE_EMAIL_NOTIFICATIONS=true
ENABLE_SMS_NOTIFICATIONS=false
ENABLE_WHATSAPP_NOTIFICATIONS=false
MANAGER_NOTIFICATION_DELAY_SECONDS=30  # Wait before sending (allow for corrections)
```

### Manager Configuration (Initial Setup)

```sql
-- Example: Assign realtors to managers
INSERT INTO realtor_managers (realtor_id, realtor_name, realtor_email, manager_id, manager_name, manager_email, manager_phone)
VALUES
    ('realtor-001', 'João Silva', 'joao@mbras.com.br', 'mgr-001', 'Maria Santos', 'maria.santos@mbras.com.br', '+5511987654321'),
    ('realtor-002', 'Ana Costa', 'ana@mbras.com.br', 'mgr-001', 'Maria Santos', 'maria.santos@mbras.com.br', '+5511987654321'),
    ('realtor-003', 'Carlos Souza', 'carlos@mbras.com.br', 'mgr-002', 'Pedro Lima', 'pedro.lima@mbras.com.br', '+5511987654322');
```

---

## API Endpoints (Future)

### Manager Configuration

**List Managers**:
```
GET /api/v1/managers
```

**Get Manager Details**:
```
GET /api/v1/managers/{manager_id}
```

**Assign Realtor to Manager**:
```
POST /api/v1/managers/{manager_id}/realtors
{
  "realtor_id": "realtor-001",
  "realtor_name": "João Silva",
  "realtor_email": "joao@mbras.com.br"
}
```

**Update Manager Notification Preferences**:
```
PUT /api/v1/managers/{manager_id}/preferences
{
  "email_enabled": true,
  "sms_enabled": false,
  "whatsapp_enabled": true,
  "notification_delay": 30,
  "notification_hours": {
    "start": "08:00",
    "end": "20:00",
    "timezone": "America/Sao_Paulo"
  }
}
```

### Notification History

**Get Manager Notifications**:
```
GET /api/v1/notifications/manager/{manager_id}?from=2025-11-01&to=2025-11-30
```

**Retry Failed Notification**:
```
POST /api/v1/notifications/{notification_id}/retry
```

---

## Testing Plan

### Unit Tests

```rust
#[tokio::test]
async fn test_handle_closed_lead() {
    let lead_data = json!({
        "id": "test-lead-001",
        "attributes": {
            "lead_status": {"alias": "won"},
            "user": {"id": "realtor-001"},
            "customer": {"name": "João Silva"}
        }
    });
    
    let result = handle_closed_lead("test-lead-001", &lead_data, &config).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_manager_notification_sent() {
    let service = ManagerNotificationService::new(&config);
    let result = service.send_notification(
        "manager@test.com",
        "lead-001",
        "João Silva",
        "won",
        &lead_data
    ).await;
    
    assert!(result.is_ok());
}
```

### Integration Tests

```bash
# 1. Create test lead in C2S
# 2. Close the lead with status "won"
# 3. Wait 60 seconds
# 4. Check manager_notifications table
psql $DB_URL -c "SELECT * FROM manager_notifications WHERE lead_id = 'test-lead-001';"

# 5. Verify email sent (check SendGrid dashboard)
# 6. Verify webhook_events status
psql $DB_URL -c "SELECT status FROM webhook_events WHERE lead_id = 'test-lead-001' AND hook_action = 'on_close_lead';"
```

---

## Success Metrics

### KPIs to Track

1. **Notification Delivery Rate**: >95% emails successfully delivered
2. **Notification Speed**: <60 seconds from lead close to manager notification
3. **Manager Engagement**: % of managers who open emails
4. **Action Rate**: % of managers who click "View Lead" link
5. **Error Rate**: <5% failed notifications

### Monitoring Queries

```sql
-- Notification success rate (last 7 days)
SELECT 
    DATE(created_at) as date,
    COUNT(*) as total,
    COUNT(*) FILTER (WHERE notification_status = 'sent') as sent,
    COUNT(*) FILTER (WHERE notification_status = 'failed') as failed,
    ROUND(100.0 * COUNT(*) FILTER (WHERE notification_status = 'sent') / COUNT(*), 2) as success_rate
FROM manager_notifications
WHERE created_at > NOW() - INTERVAL '7 days'
GROUP BY DATE(created_at)
ORDER BY date DESC;

-- Average notification time (how fast we notify after close)
SELECT 
    AVG(EXTRACT(EPOCH FROM (notification_sent_at - created_at))) as avg_seconds
FROM manager_notifications
WHERE notification_sent_at IS NOT NULL
AND created_at > NOW() - INTERVAL '7 days';
```

---

## Dependencies (Future)

### Rust Crates

```toml
[dependencies]
# Email sending
lettre = "0.11"              # SMTP email client
lettre_email = "0.9"         # Email builder

# Or use HTTP API:
sendgrid = "0.18"            # SendGrid API client

# SMS/WhatsApp
twilio = "0.7"               # Twilio API client

# Template rendering
tera = "1.19"                # Template engine (like Jinja2)
handlebars = "5.0"           # Alternative template engine

# HTML/CSS inlining
css-inline = "0.11"          # Inline CSS for email compatibility
```

---

## Security Considerations

### 1. Email Security
- Use SPF, DKIM, DMARC records
- Encrypt SMTP connections (TLS)
- Rate limit notifications (prevent spam)
- Validate manager email addresses

### 2. Data Privacy
- Don't include sensitive customer data in emails (LGPD compliance)
- Use secure links to C2S (require authentication)
- Log notification access (audit trail)
- Allow managers to opt-out

### 3. Access Control
- Only authorized users can configure managers
- Managers can only see their assigned realtors
- Role-based access (admin, manager, realtor)

---

## Rollback Plan

If manager notifications cause issues:

### 1. Disable Notifications (Keep Webhook Active)

```rust
// In src/handlers.rs
"on_close_lead" => {
    tracing::info!("Received on_close_lead webhook (notifications disabled)");
    // Skip notification logic
    Ok(())
}
```

### 2. Pause Specific Channel

```bash
# Disable email notifications only
fly secrets set ENABLE_EMAIL_NOTIFICATIONS=false -a mbras-c2s

# Re-enable when ready
fly secrets set ENABLE_EMAIL_NOTIFICATIONS=true -a mbras-c2s
```

### 3. Unsubscribe from Webhook

```bash
# Unsubscribe from on_close_lead (if API supports)
curl -X POST https://api.contact2sale.com/integration/leads/unsubscribe \
  -H "Content-Type: application/json" \
  -H "Authentication: Bearer $C2S_TOKEN" \
  -d '{"hook_action": "on_close_lead"}'
```

---

## Summary

### Current Status
- ✅ Webhook subscribed and receiving events
- ⏳ Manager notification logic pending implementation
- ⏳ Database schema pending creation
- ⏳ Email service integration pending

### Next Steps
1. Create database migrations for manager tables
2. Implement basic manager lookup service
3. Integrate email provider (SendGrid recommended)
4. Create simple email template
5. Test with real closed lead
6. Deploy to production
7. Monitor for 1 week
8. Iterate based on manager feedback

### Timeline Estimate
- **MVP**: 2-3 weeks (basic email notifications)
- **Enhanced**: 1-2 months (rich templates, multi-channel)
- **Complete**: 3+ months (analytics, dashboard, reporting)

---

**Plan Created**: 2025-11-21  
**Status**: ✅ Webhook Active, ⏳ Implementation Planned  
**Priority**: Medium (near future)  
**Owner**: Engineering Team
