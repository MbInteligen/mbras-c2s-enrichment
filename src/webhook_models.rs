use serde::{Deserialize, Serialize};
use serde_json::Value;

/// C2S Webhook Event - can be single object or array
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum WebhookPayload {
    Single(WebhookEvent),
    Batch(Vec<WebhookEvent>),
}

impl WebhookPayload {
    /// Convert to a vec of events for uniform processing
    pub fn into_events(self) -> Vec<WebhookEvent> {
        match self {
            WebhookPayload::Single(event) => vec![event],
            WebhookPayload::Batch(events) => events,
        }
    }
}

/// Individual webhook event from C2S
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebhookEvent {
    /// Lead ID
    pub id: String,

    /// Hook action type (e.g., "lead.created", "lead.updated")
    #[serde(default)]
    pub hook_action: Option<String>,

    /// Event attributes
    pub attributes: WebhookAttributes,

    /// Raw data for any additional fields
    #[serde(flatten)]
    pub raw: Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebhookAttributes {
    /// When the lead was last updated
    pub updated_at: Option<String>,

    /// Customer information
    pub customer: Option<WebhookCustomer>,

    /// Product/property information
    pub product: Option<WebhookProduct>,

    /// Lead status
    pub lead_status: Option<WebhookLeadStatus>,

    /// Log entries
    #[serde(default)]
    pub log: Vec<WebhookLogEntry>,

    /// Messages
    #[serde(default)]
    pub messages: Vec<WebhookMessage>,

    /// Raw attributes for any additional fields
    #[serde(flatten)]
    pub raw: Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebhookCustomer {
    pub name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,

    /// Raw customer data
    #[serde(flatten)]
    pub raw: Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebhookProduct {
    pub description: Option<String>,
    pub prop_ref: Option<String>,
    pub price: Option<String>,

    /// Raw product data
    #[serde(flatten)]
    pub raw: Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebhookLeadStatus {
    pub alias: Option<String>,
    pub name: Option<String>,

    /// Raw status data
    #[serde(flatten)]
    pub raw: Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebhookLogEntry {
    pub created_at: Option<String>,
    pub description: Option<String>,

    /// Raw log data
    #[serde(flatten)]
    pub raw: Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebhookMessage {
    pub created_at: Option<String>,
    pub message: Option<String>,
    pub sender: Option<String>,

    /// Raw message data
    #[serde(flatten)]
    pub raw: Value,
}

/// Response sent back to C2S webhook
#[derive(Debug, Serialize)]
pub struct WebhookResponse {
    pub status: String,
    pub received: usize,
    pub processed: usize,
    pub duplicates: usize,
}

/// Idempotency key for webhook events
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct IdempotencyKey {
    pub lead_id: String,
    pub updated_at: String,
}

impl IdempotencyKey {
    pub fn new(lead_id: String, updated_at: String) -> Self {
        Self {
            lead_id,
            updated_at,
        }
    }

    pub fn to_string(&self) -> String {
        format!("{}:{}", self.lead_id, self.updated_at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_event() {
        let json = r#"
        {
            "id": "test123",
            "hook_action": "lead.created",
            "attributes": {
                "updated_at": "2025-01-01T00:00:00Z",
                "customer": {
                    "name": "Test User",
                    "email": "test@example.com"
                }
            }
        }
        "#;

        let payload: WebhookPayload = serde_json::from_str(json).unwrap();
        match payload {
            WebhookPayload::Single(event) => {
                assert_eq!(event.id, "test123");
                assert_eq!(event.hook_action, Some("lead.created".to_string()));
            }
            _ => panic!("Expected single event"),
        }
    }

    #[test]
    fn test_parse_batch_events() {
        let json = r#"
        [
            {
                "id": "test1",
                "attributes": {"updated_at": "2025-01-01T00:00:00Z"}
            },
            {
                "id": "test2",
                "attributes": {"updated_at": "2025-01-01T00:01:00Z"}
            }
        ]
        "#;

        let payload: WebhookPayload = serde_json::from_str(json).unwrap();
        match payload {
            WebhookPayload::Batch(events) => {
                assert_eq!(events.len(), 2);
            }
            _ => panic!("Expected batch events"),
        }
    }
}
