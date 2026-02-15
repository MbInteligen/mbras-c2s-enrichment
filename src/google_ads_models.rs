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

    /// Campaign name (optional, sent by Google Ads)
    #[serde(default)]
    pub campaign_name: Option<String>,

    /// Google Ads ad group ID (adset ID)
    #[serde(default)]
    pub ad_group_id: Option<i64>,

    /// Ad group name (adset name - this is what we want to display)
    #[serde(default)]
    pub ad_group_name: Option<String>,

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
    ///
    /// Creates a clean, broker-friendly description with campaign name in Portuguese.
    /// Shows enrichment data if available, otherwise shows basic lead info.
    pub fn format_description(&self, enrichment_data: Option<&str>) -> String {
        let mut desc = String::new();

        // Test lead indicator (only if test)
        if self.is_test {
            desc.push_str("[LEAD DE TESTE]\n\n");
        }

        // Campaign/Ad group identification (no emojis - C2S DB doesn't support UTF-8 4-byte)
        let campaign_name = self.get_campaign_name();
        desc.push_str(&format!("{}\n", campaign_name));

        // Product interest (if "Compra" field exists)
        if let Some(product_field) = self
            .user_column_data
            .iter()
            .find(|f| f.column_id == "PRODUCT" || f.column_name.contains("Compra"))
        {
            desc.push_str(&format!("INTERESSE: {}\n", product_field.string_value));
        }

        desc.push_str("\n");

        // Add enrichment data (the main content) if available
        if let Some(enrichment) = enrichment_data {
            desc.push_str(enrichment);
        } else {
            // If no enrichment, show clean message for broker
            desc.push_str("Lead do Google Ads\n");
            desc.push_str("Aguardando contato do corretor");
        }

        desc.trim().to_string()
    }

    /// Get display name for the lead source
    ///
    /// Priority order:
    /// 1. Ad group name (adset name) - most specific, e.g., "Casa Jardim Europa - Condomínio"
    /// 2. Campaign name from payload - e.g., "Campanha Stoc MBRAS 2025"
    /// 3. Hardcoded campaign ID mapping - fallback for known campaigns
    /// 4. Generic format with campaign ID - last resort
    pub fn get_campaign_name(&self) -> String {
        // Priority 1: Use ad_group_name if available (most specific)
        if let Some(ad_group_name) = &self.ad_group_name {
            if !ad_group_name.trim().is_empty() {
                return ad_group_name.clone();
            }
        }

        // Priority 2: Use campaign_name from payload if available
        if let Some(campaign_name) = &self.campaign_name {
            if !campaign_name.trim().is_empty() {
                return campaign_name.clone();
            }
        }

        // Priority 3: Hardcoded campaign ID mapping (legacy fallback)
        match self.campaign_id {
            22866487607 => "MBRAS - LUX 600".to_string(),
            23184380368 => "Stoc MBRAS 2025".to_string(),
            _ => format!("Google Ads - Campanha {}", self.campaign_id),
        }
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
            campaign_name: None,
            ad_group_id: None,
            ad_group_name: None,
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
            campaign_name: None,
            ad_group_id: None,
            ad_group_name: None,
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
            campaign_name: None,
            ad_group_id: None,
            ad_group_name: None,
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

    #[test]
    fn test_get_campaign_name_priority() {
        // Test Priority 1: ad_group_name takes precedence
        let payload_with_adgroup = GoogleAdsWebhookPayload {
            lead_id: "test123".to_string(),
            api_version: "v1".to_string(),
            form_id: 123,
            campaign_id: 23184380368,
            campaign_name: Some("Campanha Stoc MBRAS 2025".to_string()),
            ad_group_id: Some(186546663977),
            ad_group_name: Some("Dona Elisa - Jardim Paulistano".to_string()),
            gcl_id: None,
            google_key: "test_key".to_string(),
            is_test: false,
            user_column_data: vec![],
        };
        assert_eq!(
            payload_with_adgroup.get_campaign_name(),
            "Dona Elisa - Jardim Paulistano"
        );

        // Test Priority 2: campaign_name used when ad_group_name is missing
        let payload_with_campaign = GoogleAdsWebhookPayload {
            lead_id: "test123".to_string(),
            api_version: "v1".to_string(),
            form_id: 123,
            campaign_id: 23184380368,
            campaign_name: Some("Campanha Stoc MBRAS 2025".to_string()),
            ad_group_id: None,
            ad_group_name: None,
            gcl_id: None,
            google_key: "test_key".to_string(),
            is_test: false,
            user_column_data: vec![],
        };
        assert_eq!(
            payload_with_campaign.get_campaign_name(),
            "Campanha Stoc MBRAS 2025"
        );

        // Test Priority 3: hardcoded mapping when both are missing
        let payload_hardcoded = GoogleAdsWebhookPayload {
            lead_id: "test123".to_string(),
            api_version: "v1".to_string(),
            form_id: 123,
            campaign_id: 23184380368,
            campaign_name: None,
            ad_group_id: None,
            ad_group_name: None,
            gcl_id: None,
            google_key: "test_key".to_string(),
            is_test: false,
            user_column_data: vec![],
        };
        assert_eq!(payload_hardcoded.get_campaign_name(), "Stoc MBRAS 2025");

        // Test Priority 4: generic format for unknown campaign
        let payload_unknown = GoogleAdsWebhookPayload {
            lead_id: "test123".to_string(),
            api_version: "v1".to_string(),
            form_id: 123,
            campaign_id: 99999999,
            campaign_name: None,
            ad_group_id: None,
            ad_group_name: None,
            gcl_id: None,
            google_key: "test_key".to_string(),
            is_test: false,
            user_column_data: vec![],
        };
        assert_eq!(
            payload_unknown.get_campaign_name(),
            "Google Ads - Campanha 99999999"
        );
    }
}
