use crate::config::Config;
use crate::errors::AppError;
use crate::models::*;
use chrono::Utc;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;

pub struct WorkApiService {
    client: Client,
    base_url: String,
    api_token: String,
}

impl WorkApiService {
    pub fn new(config: &Config) -> Self {
        Self {
            client: Client::new(),
            base_url: "https://completa.workbuscas.com".to_string(),
            api_token: config.worker_api_key.clone(),
        }
    }

    /// Fetch all available modules from Work API for a given document (CPF)
    pub async fn fetch_all_modules(
        &self,
        documento: &str,
    ) -> Result<WorkApiCompleteResponse, AppError> {
        // Using modulo=cpf returns all data at root level (DadosBasicos, DadosEconomicos, etc.)
        // Using multiple modules returns a different structure with only status/reason

        // Build URL with proper parameter encoding to prevent injection attacks
        let url = reqwest::Url::parse_with_params(
            &format!("{}/api", self.base_url),
            &[
                ("token", self.api_token.as_str()),
                ("modulo", "cpf"),
                ("consulta", documento),
            ],
        )
        .map_err(|e| AppError::ExternalApiError(format!("Failed to build URL: {}", e)))?;

        tracing::info!("Fetching all Work API modules for document: {}", documento);
        // Redact token from logs to prevent credential exposure
        tracing::debug!(
            "Work API URL: {}?token=[REDACTED]&modulo=cpf&consulta={}",
            self.base_url,
            documento
        );

        let response =
            self.client.get(url).send().await.map_err(|e| {
                AppError::ExternalApiError(format!("Work API request failed: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            tracing::error!("Work API returned error {}: {}", status, error_text);
            return Err(AppError::ExternalApiError(format!(
                "Work API returned status {}: {}",
                status, error_text
            )));
        }

        let result: WorkApiCompleteResponse = response.json().await.map_err(|e| {
            AppError::ExternalApiError(format!("Failed to parse Work API response: {}", e))
        })?;

        tracing::info!("Successfully fetched Work API modules");
        Ok(result)
    }

    /// Fetch a specific module from Work API
    pub async fn fetch_module(
        &self,
        module: &str,
        consulta: &str,
    ) -> Result<Option<Value>, AppError> {
        // Build URL with proper parameter encoding to prevent injection attacks
        let url = reqwest::Url::parse_with_params(
            &format!("{}/api", self.base_url),
            &[
                ("token", self.api_token.as_str()),
                ("modulo", module),
                ("consulta", consulta),
            ],
        )
        .map_err(|e| AppError::ExternalApiError(format!("Failed to build URL: {}", e)))?;

        tracing::info!("Fetching Work API module '{}' for: {}", module, consulta);

        let response =
            self.client.get(url).send().await.map_err(|e| {
                AppError::ExternalApiError(format!("Work API request failed: {}", e))
            })?;

        if !response.status().is_success() {
            tracing::warn!("Work API module '{}' returned non-success status", module);
            return Ok(None);
        }

        let result: Value = response.json().await.map_err(|e| {
            AppError::ExternalApiError(format!("Failed to parse Work API response: {}", e))
        })?;

        Ok(Some(result))
    }
}

pub struct CustomerService {
    pool: PgPool,
}

impl CustomerService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Find customer by CPF, email, phone, or name
    pub async fn find_customer(
        &self,
        params: &CustomerQueryParams,
    ) -> Result<Option<Customer>, AppError> {
        // Priority: CPF > Email > Phone > Name
        if let Some(ref cpf) = params.cpf {
            if let Some(customer) = self.find_by_cpf(cpf).await? {
                return Ok(Some(customer));
            }
        }

        if let Some(ref email) = params.email {
            if let Some(customer) = self.find_by_email(email).await? {
                return Ok(Some(customer));
            }
        }

        if let Some(ref phone) = params.phone {
            if let Some(customer) = self.find_by_phone(phone).await? {
                return Ok(Some(customer));
            }
        }

        if let Some(ref name) = params.name {
            if let Some(customer) = self.find_by_name(name).await? {
                return Ok(Some(customer));
            }
        }

        Ok(None)
    }

    async fn find_by_cpf(&self, cpf: &str) -> Result<Option<Customer>, AppError> {
        let customer = sqlx::query_as::<_, Customer>(
            "SELECT * FROM core.parties WHERE cpf_cnpj = $1 AND party_type = 'person' LIMIT 1",
        )
        .bind(cpf)
        .fetch_optional(&self.pool)
        .await?;

        Ok(customer)
    }

    async fn find_by_email(&self, email: &str) -> Result<Option<Customer>, AppError> {
        let result = sqlx::query_as::<_, Customer>(
            "SELECT * FROM core.parties p
             WHERE p.party_type = 'person'
               AND p.id IN (
                 SELECT pc.party_id FROM core.party_contacts pc
                 WHERE pc.contact_type::text = 'email' AND pc.value = $1
               )
             LIMIT 1",
        )
        .bind(email.to_lowercase())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!("Database error in find_by_email for '{}': {:?}", email, e);
            AppError::DatabaseError(e)
        })?;

        Ok(result)
    }

    async fn find_by_phone(&self, phone: &str) -> Result<Option<Customer>, AppError> {
        // Normalize to digits for matching stored values
        let normalized: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();
        let result = sqlx::query_as::<_, Customer>(
            "SELECT p.* FROM core.parties p
             INNER JOIN core.party_contacts pc ON p.id = pc.party_id
             WHERE pc.contact_type IN ('phone', 'whatsapp')
               AND pc.value = $1
               AND p.party_type = 'person'
             LIMIT 1",
        )
        .bind(&normalized)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    async fn find_by_name(&self, name: &str) -> Result<Option<Customer>, AppError> {
        let result = sqlx::query_as::<_, Customer>(
            "SELECT * FROM core.parties
             WHERE LOWER(full_name) LIKE LOWER($1) AND party_type = 'person'
             LIMIT 1",
        )
        .bind(format!("%{}%", name))
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    /// Get customer emails
    pub async fn get_customer_emails(
        &self,
        customer_id: &uuid::Uuid,
    ) -> Result<Vec<Email>, AppError> {
        let contacts = sqlx::query_as::<_, PartyContact>(
            r#"
            SELECT
                contact_id, party_id, contact_type::text as contact_type,
                value, is_primary, is_verified, is_whatsapp,
                source, confidence::float8, valid_from, valid_to, created_at, updated_at
            FROM core.party_contacts
            WHERE party_id = $1 AND contact_type = 'email'
            ORDER BY is_primary DESC, created_at ASC
            "#,
        )
        .bind(customer_id)
        .fetch_all(&self.pool)
        .await?;

        let emails = contacts
            .into_iter()
            .map(|pc| Email {
                id: pc.contact_id,
                email: pc.value,
                created_at: pc.created_at,
            })
            .collect();

        Ok(emails)
    }

    /// Get customer phones
    pub async fn get_customer_phones(
        &self,
        customer_id: &uuid::Uuid,
    ) -> Result<Vec<Phone>, AppError> {
        let contacts = sqlx::query_as::<_, PartyContact>(
            r#"
            SELECT
                contact_id, party_id, contact_type::text as contact_type,
                value, is_primary, is_verified, is_whatsapp,
                source, confidence::float8, valid_from, valid_to, created_at, updated_at
            FROM core.party_contacts
            WHERE party_id = $1 AND contact_type IN ('phone', 'whatsapp')
            ORDER BY is_primary DESC, created_at ASC
            "#,
        )
        .bind(customer_id)
        .fetch_all(&self.pool)
        .await?;

        let phones = contacts
            .into_iter()
            .map(|pc| Phone {
                id: pc.contact_id,
                number: pc.value,
                country_code: None,
                created_at: pc.created_at,
            })
            .collect();

        Ok(phones)
    }
}

pub struct EnrichmentService {
    work_api: WorkApiService,
    customer_service: CustomerService,
}

impl EnrichmentService {
    pub fn new(config: &Config, pool: PgPool) -> Self {
        Self {
            work_api: WorkApiService::new(config),
            customer_service: CustomerService::new(pool),
        }
    }

    /// Get or enrich customer data and return unified response
    pub async fn get_customer_unified(
        &self,
        params: &CustomerQueryParams,
    ) -> Result<UnifiedCustomerResponse, AppError> {
        let mut modules_consulted = Vec::new();
        let mut sources = Vec::new();

        // Try to find customer in local database first
        if let Some(customer) = self.customer_service.find_customer(params).await? {
            sources.push("local_db".to_string());

            let emails = self
                .customer_service
                .get_customer_emails(&customer.id)
                .await?;
            let phones = self
                .customer_service
                .get_customer_phones(&customer.id)
                .await?;

            // If customer exists but not enriched, enrich via Work API
            if !customer.enriched.unwrap_or(false) {
                let cpf = customer.cpf_cnpj.clone();
                match self.work_api.fetch_all_modules(&cpf).await {
                    Ok(work_data) => {
                        sources.push("work_api".to_string());
                        return Ok(self.build_unified_response(
                            Some(customer),
                            emails,
                            phones,
                            Some(work_data),
                            &mut modules_consulted,
                            sources,
                        ));
                    }
                    Err(e) => {
                        tracing::warn!("Failed to fetch Work API data: {:?}", e);
                    }
                }
            }

            // Return data from database only
            return Ok(self.build_unified_response(
                Some(customer),
                emails,
                phones,
                None,
                &mut modules_consulted,
                sources,
            ));
        }

        // Customer not in DB, try to fetch from Work API
        let documento = params
            .cpf
            .as_ref()
            .or(params.email.as_ref())
            .or(params.phone.as_ref())
            .ok_or_else(|| AppError::BadRequest("At least one identifier required".to_string()))?;

        match self.work_api.fetch_all_modules(documento).await {
            Ok(work_data) => {
                sources.push("work_api".to_string());
                Ok(self.build_unified_response(
                    None,
                    vec![],
                    vec![],
                    Some(work_data),
                    &mut modules_consulted,
                    sources,
                ))
            }
            Err(e) => {
                tracing::error!("Failed to fetch Work API data for new customer: {:?}", e);
                Err(AppError::NotFound(
                    "Customer not found in database or Work API".to_string(),
                ))
            }
        }
    }

    /// Build unified response from various data sources
    fn build_unified_response(
        &self,
        customer: Option<Customer>,
        emails: Vec<Email>,
        phones: Vec<Phone>,
        work_data: Option<WorkApiCompleteResponse>,
        modules_consulted: &mut Vec<String>,
        sources: Vec<String>,
    ) -> UnifiedCustomerResponse {
        let mut unified_emails = Vec::new();
        let mut unified_phones = Vec::new();
        let mut unified_addresses = Vec::new();

        // Process database data
        if let Some(ref _cust) = customer {
            for email in &emails {
                unified_emails.push(UnifiedEmail {
                    email: email.email.clone(),
                    is_valid: Some(true),
                    source: "database".to_string(),
                });
            }

            for phone in &phones {
                unified_phones.push(UnifiedPhone {
                    phone: phone.number.clone(),
                    ddd: None,
                    operator: None,
                    type_: None,
                    is_valid: Some(true),
                    source: "database".to_string(),
                });
            }
        }

        // Process Work API data
        let mut personal_info = UnifiedPersonalInfo {
            cpf: customer.as_ref().map(|c| c.cpf_cnpj.clone()),
            name: customer.as_ref().map(|c| c.full_name.clone()),
            birth_date: customer
                .as_ref()
                .and_then(|c| c.birth_date.map(|d| d.to_string())),
            gender: customer.as_ref().and_then(|c| c.sex.clone()),
            mother_name: customer.as_ref().and_then(|c| c.mother_name.clone()),
            father_name: customer.as_ref().and_then(|c| c.father_name.clone()),
            marital_status: None,
            rg: customer.as_ref().and_then(|c| c.rg.clone()),
            voter_id: None,
        };

        if let Some(ref work) = work_data {
            // Work API returns data directly at root level when using modulo=cpf
            modules_consulted.push("cpf".to_string());

            // Extract personal data from DadosBasicos
            self.extract_cpf_data(work, &mut personal_info);

            // Extract contact info from root level
            self.extract_emails(work, &mut unified_emails);
            self.extract_phones(work, &mut unified_phones);
            self.extract_addresses(work, &mut unified_addresses);
        }

        UnifiedCustomerResponse {
            source: "rust-c2s-api".to_string(),
            type_: "customer".to_string(),
            personal_info,
            contact_info: UnifiedContactInfo {
                emails: unified_emails,
                phones: unified_phones,
            },
            addresses: unified_addresses,
            financial_info: None,
            interests: None,
            wealth_assessment: None,
            metadata: ResponseMetadata {
                enriched: work_data.is_some(),
                sources,
                timestamp: Utc::now().to_rfc3339(),
                modules_consulted: modules_consulted.clone(),
            },
        }
    }

    fn extract_cpf_data(&self, data: &Value, personal_info: &mut UnifiedPersonalInfo) {
        if let Some(cpf) = data.get("cpf").and_then(|v| v.as_str()) {
            personal_info.cpf = Some(cpf.to_string());
        }
        if let Some(name) = data.get("nome").and_then(|v| v.as_str()) {
            personal_info.name = Some(name.to_string());
        }
        if let Some(birth) = data.get("nascimento").and_then(|v| v.as_str()) {
            personal_info.birth_date = Some(birth.to_string());
        }
        if let Some(gender) = data.get("sexo").and_then(|v| v.as_str()) {
            personal_info.gender = Some(gender.to_string());
        }
        if let Some(rg) = data.get("rg").and_then(|v| v.as_str()) {
            personal_info.rg = Some(rg.to_string());
        }
    }

    fn extract_emails(&self, data: &Value, emails: &mut Vec<UnifiedEmail>) {
        if let Some(email_list) = data.as_array() {
            for email_obj in email_list {
                if let Some(email) = email_obj.get("email").and_then(|v| v.as_str()) {
                    emails.push(UnifiedEmail {
                        email: email.to_string(),
                        is_valid: email_obj.get("valido").and_then(|v| v.as_bool()),
                        source: "work_api".to_string(),
                    });
                }
            }
        } else if let Some(email) = data.get("email").and_then(|v| v.as_str()) {
            emails.push(UnifiedEmail {
                email: email.to_string(),
                is_valid: None,
                source: "work_api".to_string(),
            });
        }
    }

    fn extract_phones(&self, data: &Value, phones: &mut Vec<UnifiedPhone>) {
        if let Some(phone_list) = data.as_array() {
            for phone_obj in phone_list {
                if let Some(number) = phone_obj
                    .get("telefone")
                    .or_else(|| phone_obj.get("numero"))
                    .and_then(|v| v.as_str())
                {
                    phones.push(UnifiedPhone {
                        phone: number.to_string(),
                        ddd: phone_obj
                            .get("ddd")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        operator: phone_obj
                            .get("operadora")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        type_: phone_obj
                            .get("tipo")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        is_valid: phone_obj.get("valido").and_then(|v| v.as_bool()),
                        source: "work_api".to_string(),
                    });
                }
            }
        } else if let Some(number) = data.get("telefone").and_then(|v| v.as_str()) {
            phones.push(UnifiedPhone {
                phone: number.to_string(),
                ddd: data.get("ddd").and_then(|v| v.as_str()).map(String::from),
                operator: None,
                type_: None,
                is_valid: None,
                source: "work_api".to_string(),
            });
        }
    }

    fn extract_addresses(&self, data: &Value, addresses: &mut Vec<UnifiedAddress>) {
        if let Some(address_list) = data.as_array() {
            for addr_obj in address_list {
                addresses.push(UnifiedAddress {
                    street: addr_obj
                        .get("logradouro")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    number: addr_obj
                        .get("numero")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    complement: addr_obj
                        .get("complemento")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    neighborhood: addr_obj
                        .get("bairro")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    city: addr_obj
                        .get("cidade")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    state: addr_obj
                        .get("uf")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    cep: addr_obj
                        .get("cep")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    source: "work_api".to_string(),
                });
            }
        } else {
            addresses.push(UnifiedAddress {
                street: data
                    .get("logradouro")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                number: data
                    .get("numero")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                complement: data
                    .get("complemento")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                neighborhood: data
                    .get("bairro")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                city: data
                    .get("cidade")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                state: data.get("uf").and_then(|v| v.as_str()).map(String::from),
                cep: data.get("cep").and_then(|v| v.as_str()).map(String::from),
                source: "work_api".to_string(),
            });
        }
    }
}

// ============ C2S API Integration ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct C2SLeadResponse {
    pub data: C2SLead,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct C2SLead {
    pub id: String,
    pub attributes: C2SLeadAttributes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct C2SLeadAttributes {
    pub customer: C2SCustomer,
    pub description: String,
    pub product: C2SProduct,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct C2SCustomer {
    pub id: String,
    pub name: String,
    pub email: String,
    pub phone: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct C2SProduct {
    pub prop_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct C2SMessagePayload {
    #[serde(rename = "leadId")]
    pub lead_id: String,
    pub body: String,
}

pub struct C2SService {
    client: Client,
    base_url: String,
    token: String,
}

impl C2SService {
    pub fn new(config: &Config) -> Self {
        Self {
            client: Client::new(),
            base_url: config.c2s_base_url.clone(),
            token: config.c2s_token.clone(),
        }
    }

    /// Fetch lead data from C2S by lead ID
    #[allow(dead_code)]
    pub async fn fetch_lead(&self, lead_id: &str) -> Result<C2SLeadResponse, AppError> {
        let url = format!("{}/integration/leads/{}", self.base_url, lead_id);

        tracing::info!("Fetching C2S lead: {}", lead_id);

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
                "C2S API returned status {}: {}",
                status, error_text
            )));
        }

        let lead_data: C2SLeadResponse = response.json().await.map_err(|e| {
            AppError::ExternalApiError(format!("Failed to parse C2S response: {}", e))
        })?;

        tracing::info!("Successfully fetched C2S lead: {}", lead_id);
        Ok(lead_data)
    }

    /// Send enriched data back to C2S as a message
    pub async fn send_message(&self, lead_id: &str, body: &str) -> Result<(), AppError> {
        let url = format!(
            "{}/integration/leads/{}/create_message",
            self.base_url, lead_id
        );

        let payload = C2SMessagePayload {
            lead_id: lead_id.to_string(),
            body: body.to_string(),
        };

        tracing::info!("Sending enriched data to C2S for lead: {}", lead_id);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| AppError::ExternalApiError(format!("C2S send message failed: {}", e)))?;

        if response.status().as_u16() != 201 {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(AppError::ExternalApiError(format!(
                "C2S API returned status {} (expected 201): {}",
                status, error_text
            )));
        }

        tracing::info!("Successfully sent message to C2S for lead: {}", lead_id);
        Ok(())
    }

    /// Resolve Google Ads lead source to get ad group name for product field
    /// Calls ibvi-ads-gateway /v1/leads/resolve-source endpoint
    pub async fn resolve_lead_source(&self, google_lead_id: &str) -> Result<Option<String>, AppError> {
        let gateway_url = std::env::var("C2S_GATEWAY_URL")
            .unwrap_or_else(|_| "https://mbras-c2s-gateway.fly.dev".to_string());
        
        let url = format!("{}/leads/resolve-source?google_lead_id={}", gateway_url, google_lead_id);
        
        tracing::info!("Resolving lead source for google_lead_id: {}", google_lead_id);
        
        let response = self
            .client
            .get(&url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await;
        
        match response {
            Ok(resp) if resp.status().is_success() => {
                let data: serde_json::Value = resp.json().await.map_err(|e| {
                    AppError::ExternalApiError(format!("Failed to parse resolve-source response: {}", e))
                })?;
                
                // Extract product_description from response
                if let Some(product_desc) = data.get("product_description").and_then(|v| v.as_str()) {
                    tracing::info!("✅ Resolved product: {}", product_desc);
                    Ok(Some(product_desc.to_string()))
                } else if let Some(ad_group_name) = data.get("ad_group_name").and_then(|v| v.as_str()) {
                    tracing::info!("✅ Resolved ad_group_name: {}", ad_group_name);
                    Ok(Some(ad_group_name.to_string()))
                } else {
                    tracing::warn!("⚠️  resolve-source returned no product_description");
                    Ok(None)
                }
            }
            Ok(resp) => {
                let status = resp.status();
                let error_text = resp.text().await.unwrap_or_default();
                tracing::warn!("⚠️  resolve-source failed {}: {}", status, error_text);
                Ok(None)
            }
            Err(e) => {
                tracing::warn!("⚠️  resolve-source request failed: {}", e);
                Ok(None)
            }
        }
    }

    /// Create a new lead in C2S
    pub async fn create_lead(
        &self,
        customer_name: &str,
        phone: Option<&str>,
        email: Option<&str>,
        description: &str,
        source: Option<&str>,
        product: Option<&str>,
        seller_id: Option<&str>,
    ) -> Result<String, AppError> {
        let url = format!("{}/integration/leads", self.base_url);

        // Build attributes using JSON:API format
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
        if let Some(product_val) = product {
            attributes.insert("product".to_string(), json!(product_val));
        }
        if let Some(seller_val) = seller_id {
            attributes.insert("seller_id".to_string(), json!(seller_val));
        }

        let payload = json!({
            "data": {
                "type": "lead",
                "attributes": attributes
            }
        });

        tracing::info!("Creating new lead in C2S: {}", customer_name);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| AppError::ExternalApiError(format!("C2S create lead failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(AppError::ExternalApiError(format!(
                "C2S API create lead failed {}: {}",
                status, error_text
            )));
        }

        let response_data: serde_json::Value = response.json().await.map_err(|e| {
            AppError::ExternalApiError(format!("Failed to parse C2S create lead response: {}", e))
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
            // Try numeric IDs converted to string
            if let Some(id) = response_data
                .get("data")
                .and_then(|d| d.get("id"))
                .and_then(|i| i.as_i64())
            {
                id.to_string()
            } else if let Some(id) = response_data.get("id").and_then(|i| i.as_i64()) {
                id.to_string()
            } else {
                return Err(AppError::ExternalApiError(
                    "C2S response missing 'id', 'data.id' or 'lead_id' field".to_string(),
                ));
            }
        };

        tracing::info!("✅ Created lead in C2S: {}", lead_id);
        Ok(lead_id)
    }
}

// ============ Diretrix API Integration ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiretrixPersonSearch {
    pub nome: String,
    pub cpf: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiretrixPersonData {
    pub nome: String,
    pub cpf: String,
    pub rg: Option<String>,
    #[serde(rename = "rgOrgaoEmissor")]
    pub rg_orgao_emissor: Option<String>,
    #[serde(rename = "dataNascimento")]
    pub data_nascimento: Option<String>,
    pub idade: Option<String>,
    pub signo: Option<String>,
    pub sexo: Option<String>,
    pub mae: Option<String>,
    pub telefones: Vec<DiretrixPhone>,
    pub emails: Vec<DiretrixEmail>,
    pub enderecos: Vec<DiretrixAddress>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiretrixPhone {
    pub numero: String,
    pub ddd: String,
    pub operadora: Option<String>,
    pub tipo: Option<String>,
    pub ranking: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiretrixEmail {
    pub endereco: String,
    pub ranking: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiretrixAddress {
    pub logadouro: String,
    pub numero: String,
    pub bairro: String,
    pub cidade: String,
    pub uf: String,
    pub cep: String,
    pub complemento: Option<String>,
    pub ranking: i32,
    #[serde(rename = "logadouroTipo")]
    pub logadouro_tipo: Option<String>,
}

pub struct DiretrixService {
    client: Client,
    base_url: String,
    username: String,
    password: String,
}

impl DiretrixService {
    pub fn new(config: &Config) -> Self {
        Self {
            client: Client::new(),
            base_url: config.diretrix_base_url.clone(),
            username: config.diretrix_user.clone(),
            password: config.diretrix_pass.clone(),
        }
    }

    /// Search person by phone number - returns list of possible matches
    pub async fn search_by_phone(
        &self,
        phone: &str,
    ) -> Result<Vec<DiretrixPersonSearch>, AppError> {
        // Remove 55 prefix if present (Diretrix expects phone without country code)
        let phone_clean = if phone.starts_with("55") && phone.len() > 2 {
            &phone[2..]
        } else {
            phone
        };

        let url = format!(
            "{}/Consultas/Pessoa/Telefone/{}",
            self.base_url, phone_clean
        );

        tracing::info!(
            "Diretrix: Searching by phone: {} (cleaned: {})",
            phone,
            phone_clean
        );

        let response = self
            .client
            .get(&url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await
            .map_err(|e| {
                AppError::ExternalApiError(format!("Diretrix phone search failed: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(AppError::ExternalApiError(format!(
                "Diretrix API returned status {}: {}",
                status, error_text
            )));
        }

        let results: Vec<DiretrixPersonSearch> = response.json().await.map_err(|e| {
            AppError::ExternalApiError(format!("Failed to parse Diretrix phone response: {}", e))
        })?;

        tracing::info!(
            "Diretrix: Found {} matches for phone {}",
            results.len(),
            phone
        );
        Ok(results)
    }

    /// Search person by email - returns list of possible matches
    pub async fn search_by_email(
        &self,
        email: &str,
    ) -> Result<Vec<DiretrixPersonSearch>, AppError> {
        let url = format!("{}/Consultas/Pessoa/Email/{}", self.base_url, email);

        tracing::info!("Diretrix: Searching by email: {}", email);

        let response = self
            .client
            .get(&url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await
            .map_err(|e| {
                AppError::ExternalApiError(format!("Diretrix email search failed: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            return Err(AppError::ExternalApiError(format!(
                "Diretrix API returned status {}",
                status
            )));
        }

        let results: Vec<DiretrixPersonSearch> = response.json().await.map_err(|e| {
            AppError::ExternalApiError(format!("Failed to parse Diretrix email response: {}", e))
        })?;

        tracing::info!(
            "Diretrix: Found {} matches for email {}",
            results.len(),
            email
        );
        Ok(results)
    }

    /// Get full person data by CPF
    #[allow(dead_code)]
    pub async fn get_person_by_cpf(&self, cpf: &str) -> Result<DiretrixPersonData, AppError> {
        let url = format!("{}/Consultas/Pessoa/{}", self.base_url, cpf);

        tracing::info!("Diretrix: Getting person data for CPF: {}", cpf);

        let response = self
            .client
            .get(&url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await
            .map_err(|e| {
                AppError::ExternalApiError(format!("Diretrix CPF lookup failed: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(AppError::ExternalApiError(format!(
                "Diretrix API returned status {}: {}",
                status, error_text
            )));
        }

        let person_data: DiretrixPersonData = response.json().await.map_err(|e| {
            AppError::ExternalApiError(format!("Failed to parse Diretrix person data: {}", e))
        })?;

        tracing::info!(
            "Diretrix: Successfully retrieved data for {}",
            person_data.nome
        );
        Ok(person_data)
    }

    /// Enrich person data - search by phone/email, then get full data by CPF
    #[allow(dead_code)]
    pub async fn enrich_person(
        &self,
        phone: Option<&str>,
        email: Option<&str>,
    ) -> Result<Option<DiretrixPersonData>, AppError> {
        // Try phone first
        if let Some(phone_num) = phone {
            match self.search_by_phone(phone_num).await {
                Ok(results) if !results.is_empty() => {
                    let cpf = &results[0].cpf;
                    return Ok(Some(self.get_person_by_cpf(cpf).await?));
                }
                _ => {}
            }
        }

        // Try email if phone didn't work
        if let Some(email_addr) = email {
            match self.search_by_email(email_addr).await {
                Ok(results) if !results.is_empty() => {
                    let cpf = &results[0].cpf;
                    return Ok(Some(self.get_person_by_cpf(cpf).await?));
                }
                _ => {}
            }
        }

        Ok(None)
    }
}
