use crate::errors::AppError;
use reqwest;
use serde_json::json;
use std::time::Duration;
use tracing;

/// Client for interacting directly with the C2S API.
///
/// Formerly communicated via a Python Gateway, now direct.
#[derive(Clone)]
pub struct C2sGatewayClient {
    client: reqwest::Client,
    base_url: String,
    token: String,
}

impl C2sGatewayClient {
    /// Creates a new `C2sGatewayClient`.
    ///
    /// # Arguments
    ///
    /// * `base_url` - The base URL of the C2S API.
    /// * `token` - The API token for authentication.
    #[allow(dead_code)]
    pub fn new(base_url: String, token: String) -> Result<Self, AppError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| {
                AppError::ExternalApiError(format!("Failed to create C2S client: {}", e))
            })?;

        Ok(Self {
            client,
            base_url,
            token,
        })
    }

    /// Gets a lead from C2S.
    ///
    /// # Arguments
    ///
    /// * `lead_id` - The ID of the lead to fetch.
    ///
    /// # Returns
    ///
    /// * `Result<serde_json::Value, AppError>` - The lead data.
    pub async fn get_lead(&self, lead_id: &str) -> Result<serde_json::Value, AppError> {
        let url = format!("{}/integration/leads/{}", self.base_url, lead_id);
        tracing::info!("Fetching lead {} from C2S: {}", lead_id, url);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await
            .map_err(|e| AppError::ExternalApiError(format!("C2S request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(AppError::ExternalApiError(format!(
                "C2S returned {}: {}",
                status, error_text
            )));
        }

        let data = response.json().await.map_err(|e| {
            AppError::ExternalApiError(format!("Failed to parse C2S response: {}", e))
        })?;

        Ok(data)
    }

    /// Creates a new lead in C2S.
    ///
    /// # Arguments
    ///
    /// * `customer_name` - Name of the customer.
    /// * `phone` - Optional phone number.
    /// * `email` - Optional email address.
    /// * `description` - Lead description.
    /// * `source` - Source of the lead (defaults to "Google Ads").
    /// * `seller_id` - Optional seller ID to assign.
    ///
    /// # Returns
    ///
    /// * `Result<String, AppError>` - The ID of the created lead.
    #[allow(dead_code)]
    pub async fn create_lead(
        &self,
        customer_name: &str,
        phone: Option<&str>,
        email: Option<&str>,
        description: &str,
        source: Option<&str>,
        seller_id: Option<&str>,
    ) -> Result<String, AppError> {
        let url = format!("{}/integration/leads", self.base_url);
        tracing::info!("Creating new lead in C2S: {}", customer_name);

        // Build attributes object
        let mut attributes = serde_json::Map::new();
        attributes.insert("name".to_string(), json!(customer_name));
        attributes.insert("description".to_string(), json!(description));
        attributes.insert("type_negotiation".to_string(), json!("Compra"));
        attributes.insert("source".to_string(), json!(source.unwrap_or("Google Ads")));

        if let Some(phone_val) = phone {
            attributes.insert("phone".to_string(), json!(phone_val));
        }
        if let Some(email_val) = email {
            attributes.insert("email".to_string(), json!(email_val));
        }
        if let Some(seller_val) = seller_id {
            attributes.insert("seller_id".to_string(), json!(seller_val));
        }

        // Use JSON:API format (C2S requirement)
        let body = json!({
            "data": {
                "type": "lead",
                "attributes": attributes
            }
        });

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::ExternalApiError(format!("Failed to create lead: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(AppError::ExternalApiError(format!(
                "C2S lead creation failed {}: {}",
                status, error_text
            )));
        }

        let response_data: serde_json::Value = response.json().await.map_err(|e| {
            AppError::ExternalApiError(format!("Failed to parse lead creation response: {}", e))
        })?;

        // Try to get ID from different possible locations in response
        let lead_id = if let Some(id) = response_data
            .get("data")
            .and_then(|d| d.get("id"))
            .and_then(|i| i.as_str())
        {
            id.to_string()
        } else if let Some(id) = response_data.get("id").and_then(|i| i.as_str()) {
            id.to_string()
        } else if let Some(id) = response_data.get("lead_id").and_then(|i| i.as_str()) {
            id.to_string()
        } else {
            // Fallback: check if it's a number and convert to string
            if let Some(id) = response_data
                .get("data")
                .and_then(|d| d.get("id"))
                .and_then(|i| i.as_i64())
            {
                id.to_string()
            } else if let Some(id) = response_data.get("id").and_then(|i| i.as_i64()) {
                id.to_string()
            } else {
                tracing::warn!("Unexpected C2S response format: {:?}", response_data);
                return Err(AppError::ExternalApiError(
                    "Lead creation response missing 'id' field".to_string(),
                ));
            }
        };

        tracing::info!("✓ Lead created successfully: {}", lead_id);
        Ok(lead_id)
    }

    /// Sends a message to a lead in C2S.
    ///
    /// # Arguments
    ///
    /// * `lead_id` - The ID of the lead.
    /// * `message` - The message content.
    ///
    /// # Returns
    ///
    /// * `Result<(), AppError>` - Ok or an error.
    pub async fn send_message(&self, lead_id: &str, message: &str) -> Result<(), AppError> {
        let url = format!(
            "{}/integration/leads/{}/create_message",
            self.base_url, lead_id
        );
        tracing::info!("Sending message to lead {} in C2S", lead_id);

        // C2S expects { "leadId": "...", "body": "..." }
        let body = json!({
            "leadId": lead_id,
            "body": message
        });

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::ExternalApiError(format!("Failed to send message: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(AppError::ExternalApiError(format!(
                "C2S message send failed {}: {}",
                status, error_text
            )));
        }

        tracing::info!("✓ Message sent successfully to lead {}", lead_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_creation() {
        let client = C2sGatewayClient::new("https://example.com".to_string(), "token".to_string());
        assert!(client.is_ok());
    }
}
