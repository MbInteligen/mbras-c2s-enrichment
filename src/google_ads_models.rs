use serde::{Deserialize, Serialize};

/// Google Ads Lead Form webhook payload
/// Documentation: https://developers.google.com/google-ads/api/docs/leads/webhooks
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GoogleAdsWebhookPayload {
    /// Unique lead identifier (used for deduplication)
    pub lead_id: String,

    /// API version
    pub api_version: String,

    /// Google Ads form ID
    pub form_id: i64,

    /// Google Ads campaign ID
    pub campaign_id: i64,

    /// Google Click ID (gcl_id) for conversion tracking
    #[serde(default)]
    pub gcl_id: Option<String>,

    /// Webhook verification key (REQUIRED for security)
    pub google_key: String,

    /// Whether this is a test lead
    pub is_test: bool,

    /// Dynamic form fields submitted by the user
    pub user_column_data: Vec<UserColumnData>,
}

/// Individual form field data
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UserColumnData {
    /// Column identifier (e.g., "FULL_NAME", "EMAIL", "PHONE_NUMBER")
    pub column_id: String,

    /// Human-readable column name
    pub column_name: String,

    /// User-submitted value
    pub string_value: String,
}

impl GoogleAdsWebhookPayload {
    /// Extract full name from form data
    pub fn get_name(&self) -> Option<String> {
        self.user_column_data
            .iter()
            .find(|field| field.column_id == "FULL_NAME" || field.column_id == "NAME")
            .map(|field| field.string_value.clone())
    }

    /// Extract email from form data
    pub fn get_email(&self) -> Option<String> {
        self.user_column_data
            .iter()
            .find(|field| field.column_id == "EMAIL")
            .map(|field| field.string_value.trim().to_lowercase())
    }

    /// Extract phone number from form data
    pub fn get_phone(&self) -> Option<String> {
        self.user_column_data
            .iter()
            .find(|field| field.column_id == "PHONE_NUMBER" || field.column_id == "PHONE")
            .map(|field| field.string_value.clone())
    }

    /// Extract CPF from form data (if form includes CPF field)
    pub fn get_cpf(&self) -> Option<String> {
        self.user_column_data
            .iter()
            .find(|field| {
                field.column_id == "CPF"
                    || field.column_id == "DOCUMENT"
                    || field.column_name.to_lowercase().contains("cpf")
            })
            .map(|field| {
                // Remove formatting: 123.456.789-01 -> 12345678901
                field
                    .string_value
                    .chars()
                    .filter(|c| c.is_numeric())
                    .collect()
            })
    }

    /// Extract city from form data
    #[allow(dead_code)]
    pub fn get_city(&self) -> Option<String> {
        self.user_column_data
            .iter()
            .find(|field| {
                field.column_id == "CITY" || field.column_name.to_lowercase().contains("cidade")
            })
            .map(|field| field.string_value.clone())
    }

    /// Extract custom field by column_id
    #[allow(dead_code)]
    pub fn get_field(&self, column_id: &str) -> Option<String> {
        self.user_column_data
            .iter()
            .find(|field| field.column_id == column_id)
            .map(|field| field.string_value.clone())
    }

    /// Generate formatted description for C2S
    pub fn format_description(&self, enrichment_data: Option<&str>) -> String {
        let mut desc = String::new();

        // Google Ads context
        desc.push_str("🎯 Lead do Google Ads\n\n");
        desc.push_str(&format!("📊 Campanha ID: {}\n", self.campaign_id));
        desc.push_str(&format!("📝 Formulário ID: {}\n", self.form_id));

        if let Some(gcl_id) = &self.gcl_id {
            desc.push_str(&format!("🔗 GCLID: {}\n", gcl_id));
        }

        if self.is_test {
            desc.push_str("⚠️  LEAD DE TESTE\n");
        }

        desc.push_str("\n📋 Informações do Formulário:\n");
        for field in &self.user_column_data {
            desc.push_str(&format!(
                "   • {}: {}\n",
                field.column_name, field.string_value
            ));
        }

        // Add enrichment data if available
        if let Some(enrichment) = enrichment_data {
            desc.push_str("\n");
            desc.push_str(enrichment);
        }

        desc
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_name() {
        let payload = GoogleAdsWebhookPayload {
            lead_id: "test123".to_string(),
            api_version: "v1".to_string(),
            form_id: 123,
            campaign_id: 456,
            gcl_id: None,
            google_key: "test_key".to_string(),
            is_test: true,
            user_column_data: vec![UserColumnData {
                column_id: "FULL_NAME".to_string(),
                column_name: "Nome Completo".to_string(),
                string_value: "João Silva".to_string(),
            }],
        };

        assert_eq!(payload.get_name(), Some("João Silva".to_string()));
    }

    #[test]
    fn test_extract_email() {
        let payload = GoogleAdsWebhookPayload {
            lead_id: "test123".to_string(),
            api_version: "v1".to_string(),
            form_id: 123,
            campaign_id: 456,
            gcl_id: None,
            google_key: "test_key".to_string(),
            is_test: true,
            user_column_data: vec![UserColumnData {
                column_id: "EMAIL".to_string(),
                column_name: "E-mail".to_string(),
                string_value: "  JOAO@EXAMPLE.COM  ".to_string(),
            }],
        };

        assert_eq!(payload.get_email(), Some("joao@example.com".to_string()));
    }

    #[test]
    fn test_extract_cpf() {
        let payload = GoogleAdsWebhookPayload {
            lead_id: "test123".to_string(),
            api_version: "v1".to_string(),
            form_id: 123,
            campaign_id: 456,
            gcl_id: None,
            google_key: "test_key".to_string(),
            is_test: true,
            user_column_data: vec![UserColumnData {
                column_id: "CPF".to_string(),
                column_name: "CPF".to_string(),
                string_value: "123.456.789-01".to_string(),
            }],
        };

        assert_eq!(payload.get_cpf(), Some("12345678901".to_string()));
    }
}
