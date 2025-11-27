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

/// Attributes associated with a webhook event.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebhookAttributes {
    /// When the lead was last updated.
    pub updated_at: Option<String>,

    /// Customer information.
    pub customer: Option<WebhookCustomer>,

    /// Product/property information.
    pub product: Option<WebhookProduct>,

    /// Lead status.
    pub lead_status: Option<WebhookLeadStatus>,

    /// Log entries associated with the lead.
    #[serde(default)]
    pub log: Vec<WebhookLogEntry>,

    /// Messages exchanged with the lead.
    #[serde(default)]
    pub messages: Vec<WebhookMessage>,

    /// Raw attributes for any additional fields not explicitly mapped.
    #[serde(flatten)]
    pub raw: Value,
}

/// Customer information extracted from webhook attributes.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebhookCustomer {
    /// Name of the customer.
    pub name: Option<String>,
    /// Email address of the customer.
    pub email: Option<String>,
    /// Phone number of the customer.
    pub phone: Option<String>,

    /// Raw customer data not explicitly mapped.
    #[serde(flatten)]
    pub raw: Value,
}

/// Product or property information associated with the lead.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebhookProduct {
    /// Description of the product.
    pub description: Option<String>,
    /// Property reference code.
    pub prop_ref: Option<String>,
    /// Price of the product.
    pub price: Option<String>,

    /// Raw product data not explicitly mapped.
    #[serde(flatten)]
    pub raw: Value,
}

/// Status of the lead in the C2S pipeline.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebhookLeadStatus {
    /// Status alias (internal identifier).
    pub alias: Option<String>,
    /// Display name of the status.
    pub name: Option<String>,

    /// Raw status data not explicitly mapped.
    #[serde(flatten)]
    pub raw: Value,
}

/// Log entry associated with the lead.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebhookLogEntry {
    /// When the log entry was created.
    pub created_at: Option<String>,
    /// Description of the log event.
    pub description: Option<String>,

    /// Raw log data not explicitly mapped.
    #[serde(flatten)]
    pub raw: Value,
}

/// Message exchanged with the lead.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebhookMessage {
    /// When the message was created.
    pub created_at: Option<String>,
    /// Content of the message.
    pub message: Option<String>,
    /// Sender of the message.
    pub sender: Option<String>,

    /// Raw message data not explicitly mapped.
    #[serde(flatten)]
    pub raw: Value,
}

/// Response sent back to C2S after processing a webhook payload.
#[derive(Debug, Serialize)]
pub struct WebhookResponse {
    /// Status of the operation (e.g., "success", "partial_success").
    pub status: String,
    /// Number of events received.
    pub received: usize,
    /// Number of events successfully processed.
    pub processed: usize,
    /// Number of duplicate events skipped.
    pub duplicates: usize,
}

/// Idempotency key for deduplicating webhook events.
///
/// This key combines the lead ID and the updated_at timestamp to ensure
/// we don't process the same update multiple times.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct IdempotencyKey {
    /// Unique identifier of the lead.
    pub lead_id: String,
    /// Timestamp of the last update.
    pub updated_at: String,
}

impl IdempotencyKey {
    /// Creates a new idempotency key.
    ///
    /// # Arguments
    ///
    /// * `lead_id` - Unique identifier of the lead.
    /// * `updated_at` - Timestamp of the last update.
    #[allow(dead_code)]
    pub fn new(lead_id: String, updated_at: String) -> Self {
        Self {
            lead_id,
            updated_at,
        }
    }

    /// Converts the key to a string representation.
    ///
    /// # Returns
    ///
    /// * `String` - A string in the format "lead_id:updated_at".
    #[allow(dead_code)]
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
