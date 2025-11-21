use crate::errors::AppError;
use reqwest;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;
use tracing;

/// Client for interacting with the Python C2S Gateway
/// This provides a cleaner interface than direct C2S API calls
#[derive(Clone)]
pub struct C2sGatewayClient {
    client: reqwest::Client,
    base_url: String,
}

impl C2sGatewayClient {
    pub fn new(base_url: String) -> Result<Self, AppError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| {
                AppError::ExternalApiError(format!("Failed to create gateway client: {}", e))
            })?;

        Ok(Self { client, base_url })
    }

    /// Health check the gateway
    pub async fn health_check(&self) -> Result<serde_json::Value, AppError> {
        let response = self.client.get(&self.base_url).send().await?.json().await?;

        Ok(response)
    }

    /// Get lead from C2S via gateway
    pub async fn get_lead(&self, lead_id: &str) -> Result<serde_json::Value, AppError> {
        let url = format!("{}/leads/{}", self.base_url, lead_id);
        tracing::info!("Fetching lead {} via gateway: {}", lead_id, url);

        let response =
            self.client.get(&url).send().await.map_err(|e| {
                AppError::ExternalApiError(format!("Gateway request failed: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(AppError::ExternalApiError(format!(
                "Gateway returned {}: {}",
                status, error_text
            )));
        }

        let data = response.json().await.map_err(|e| {
            AppError::ExternalApiError(format!("Failed to parse gateway response: {}", e))
        })?;

        Ok(data)
    }

    /// Send message to lead via gateway
    pub async fn send_message(&self, lead_id: &str, message: &str) -> Result<(), AppError> {
        let url = format!("{}/leads/{}/messages", self.base_url, lead_id);
        tracing::info!("Sending message to lead {} via gateway", lead_id);

        let body = json!({
            "message": message
        });

        let response = self
            .client
            .post(&url)
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
                "Gateway message send failed {}: {}",
                status, error_text
            )));
        }

        tracing::info!("✓ Message sent successfully to lead {}", lead_id);
        Ok(())
    }

    /// Update lead via gateway
    pub async fn update_lead(
        &self,
        lead_id: &str,
        data: serde_json::Value,
    ) -> Result<(), AppError> {
        let url = format!("{}/leads/{}", self.base_url, lead_id);
        tracing::info!("Updating lead {} via gateway", lead_id);

        let response = self
            .client
            .patch(&url)
            .json(&data)
            .send()
            .await
            .map_err(|e| AppError::ExternalApiError(format!("Failed to update lead: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(AppError::ExternalApiError(format!(
                "Gateway update failed {}: {}",
                status, error_text
            )));
        }

        tracing::info!("✓ Lead {} updated successfully", lead_id);
        Ok(())
    }

    /// List leads via gateway (with optional filters)
    pub async fn list_leads(
        &self,
        filters: Option<LeadFilters>,
    ) -> Result<Vec<serde_json::Value>, AppError> {
        let mut url = format!("{}/leads", self.base_url);

        // Add query parameters if filters provided
        if let Some(f) = filters {
            let mut params = vec![];
            if let Some(status) = f.status {
                params.push(format!("status={}", status));
            }
            if let Some(phone) = f.phone {
                params.push(format!("phone={}", phone));
            }
            if let Some(email) = f.email {
                params.push(format!("email={}", email));
            }
            if !params.is_empty() {
                url = format!("{}?{}", url, params.join("&"));
            }
        }

        tracing::info!("Listing leads via gateway: {}", url);

        let response = self.client.get(&url).send().await?.json().await?;

        Ok(response)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LeadFilters {
    pub status: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_gateway_client_creation() {
        let client = C2sGatewayClient::new("https://example.com".to_string());
        assert!(client.is_ok());
    }
}
