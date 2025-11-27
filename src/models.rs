use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// ============ Database Models ============

/// Represents a party (person or company) in the system.
///
/// This is the central entity for storing customer information.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Party {
    /// Unique identifier for the party.
    pub id: Uuid,
    /// Type of the party (e.g., "PERSON", "COMPANY").
    pub party_type: String,
    /// CPF or CNPJ document number.
    pub cpf_cnpj: String,
    /// Full name or legal name.
    pub full_name: String,
    /// Normalized name for search purposes.
    pub normalized_name: Option<String>,
    /// Gender (e.g., "M", "F").
    pub sex: Option<String>,
    /// Date of birth.
    pub birth_date: Option<NaiveDate>,
    /// Mother's name.
    pub mother_name: Option<String>,
    /// Father's name.
    pub father_name: Option<String>,
    /// RG (General Registry) number.
    pub rg: Option<String>,
    /// Fantasy name (for companies).
    pub fantasy_name: Option<String>,
    /// Normalized fantasy name.
    pub normalized_fantasy_name: Option<String>,
    /// Date of opening (for companies).
    pub opening_date: Option<NaiveDate>,
    /// Date of registration status.
    pub registration_status_date: Option<NaiveDate>,
    /// Type of company (e.g., "LTDA", "MEI").
    pub company_type: Option<String>,
    /// Size of the company.
    pub company_size: Option<String>,
    /// Whether the party has been enriched with external data.
    pub enriched: Option<bool>,
    /// Timestamp of creation.
    pub created_at: DateTime<Utc>,
    /// Timestamp of last update.
    pub updated_at: Option<DateTime<Utc>>,
}

/// Alias for `Party` for backward compatibility.
pub type Customer = Party;

/// Represents person-specific attributes linked to a `Party`.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Person {
    /// Foreign key to the `Party` table.
    pub party_id: Uuid,
    /// Full name of the person.
    pub full_name: String,
    /// Mother's name.
    pub mothers_name: Option<String>,
    /// Date of birth.
    pub birth_date: Option<NaiveDate>,
    /// Gender.
    pub sex: Option<String>,
    /// Marital status.
    pub marital_status: Option<String>,
    /// CPF document number.
    pub document_cpf: Option<String>,
    /// Timestamp of creation.
    pub created_at: DateTime<Utc>,
    /// Timestamp of last update.
    pub updated_at: Option<DateTime<Utc>>,
}

/// Represents company-specific attributes linked to a `Party`.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Company {
    /// Foreign key to the `Party` table.
    pub party_id: Uuid,
    /// Legal name of the company.
    pub legal_name: String,
    /// Trade name (fantasy name).
    pub trade_name: Option<String>,
    /// CNPJ document number.
    pub cnpj: Option<String>,
    /// Size of the company.
    pub company_size: Option<String>,
    /// Industry sector.
    pub industry: Option<String>,
    /// Date of foundation.
    pub foundation_date: Option<NaiveDate>,
    /// Timestamp of creation.
    pub created_at: DateTime<Utc>,
    /// Timestamp of last update.
    pub updated_at: Option<DateTime<Utc>>,
}

/// Represents a contact method (phone, email, etc.) for a `Party`.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct PartyContact {
    /// Unique identifier for the contact.
    pub contact_id: Uuid,
    /// Foreign key to the `Party` table.
    pub party_id: Uuid,
    /// Type of contact (e.g., "EMAIL", "PHONE").
    pub contact_type: String,
    /// Contact value (e.g., the email address or phone number).
    pub value: String,
    /// Whether this is the primary contact.
    pub is_primary: bool,
    /// Whether the contact has been verified.
    pub is_verified: bool,
    /// Whether the contact is a WhatsApp number (for phones).
    pub is_whatsapp: bool,
    /// Source of the contact data.
    pub source: Option<String>,
    /// Confidence score of the contact information.
    pub confidence: Option<f64>,
    /// Start date of validity.
    pub valid_from: Option<DateTime<Utc>>,
    /// End date of validity.
    pub valid_to: Option<DateTime<Utc>>,
    /// Timestamp of creation.
    pub created_at: DateTime<Utc>,
    /// Timestamp of last update.
    pub updated_at: Option<DateTime<Utc>>,
}

/// Represents an email address.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Email {
    /// Unique identifier.
    pub id: Uuid,
    /// Email address.
    pub email: String,
    /// Timestamp of creation.
    pub created_at: DateTime<Utc>,
}

/// Represents a phone number.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Phone {
    /// Unique identifier.
    pub id: Uuid,
    /// Phone number.
    pub number: String,
    /// Country code.
    pub country_code: Option<String>,
    /// Timestamp of creation.
    pub created_at: DateTime<Utc>,
}

// ============ API Request/Response Models ============

/// Request payload for processing a lead.
#[derive(Debug, Deserialize)]
pub struct LeadRequest {
    /// ID of the lead from the source system.
    pub lead_id: String,
    /// Personal information of the lead.
    pub personal_info: PersonalInfo,
    /// Contact information of the lead.
    pub contact_info: ContactInfo,
}

/// Personal information in a lead request.
#[derive(Debug, Deserialize)]
pub struct PersonalInfo {
    /// Name of the person.
    pub name: String,
    /// Email address.
    pub email: Option<String>,
    /// CPF document number.
    pub cpf: Option<String>,
}

/// Contact information in a lead request.
#[derive(Debug, Deserialize)]
pub struct ContactInfo {
    /// List of phone numbers.
    pub phones: Vec<PhoneInfo>,
}

/// Phone information in a lead request.
#[derive(Debug, Deserialize)]
pub struct PhoneInfo {
    /// Phone number.
    pub phone: String,
}

/// Response payload for lead processing operations.
#[derive(Debug, Serialize)]
pub struct LeadResponse {
    /// Whether the operation was successful.
    pub success: bool,
    /// Message describing the result.
    pub message: String,
    /// Optional enriched data returned.
    pub data: Option<EnrichedCustomerData>,
}

// ============ Lookup Response (matches Go LookupResponse) ============

/// Response structure for enrichment lookup, matching the format of the Go service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupResponse {
    /// Source of the data.
    pub source: String,
    /// Type of the entity.
    #[serde(rename = "type")]
    pub type_: String,
    /// Personal information.
    pub personal_info: LookupPersonalInfo,
    /// Contact information.
    pub contact_info: LookupContactInfo,
    /// List of addresses.
    pub addresses: Vec<LookupAddress>,
    /// Financial information.
    pub financial_info: LookupFinancialInfo,
    /// Job history (dynamic JSON).
    pub jobs: Vec<serde_json::Value>,
    /// Vehicles owned (dynamic JSON).
    pub vehicles: Vec<serde_json::Value>,
    /// Interest profile.
    pub interests: LookupInterests,
    /// Purchase history (dynamic JSON).
    pub purchase_history: Option<serde_json::Value>,
    /// Education history.
    pub educations: Vec<LookupEducation>,
}

/// Personal information from lookup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupPersonalInfo {
    /// CPF document.
    pub cpf: String,
    /// Full name.
    pub name: String,
    /// Date of birth.
    pub birth_date: Option<String>,
    /// Gender.
    pub gender: Option<String>,
    /// Mother's name.
    #[allow(dead_code)]
    pub mother_name: Option<String>,
    /// Father's name.
    pub father_name: Option<String>,
    /// Marital status.
    pub marital_status: Option<String>,
    /// Nationality.
    pub nationality: Option<String>,
    /// RG document.
    pub rg: Option<String>,
    /// Voter ID.
    pub voter_id: Option<String>,
}

/// Contact information from lookup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupContactInfo {
    /// List of emails.
    pub emails: Vec<LookupEmail>,
    /// List of phones.
    pub phones: Vec<LookupPhone>,
}

/// Email details from lookup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupEmail {
    /// Unique identifier.
    pub id: String,
    /// Email address.
    pub email: String,
    /// Whether the email is valid.
    pub is_valid: bool,
    /// Ranking of the email.
    pub ranking: i32,
    /// Quality score of the data.
    pub quality_score: f64,
    /// Creation timestamp.
    pub created_at: String,
    /// Update timestamp.
    pub updated_at: String,
}

/// Phone details from lookup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupPhone {
    /// Unique identifier.
    pub id: String,
    /// Phone number.
    pub phone: String,
    /// Area code (DDD).
    pub ddd: String,
    /// Telecom operator.
    pub operator: Option<String>,
    /// Type of phone (e.g., "MOBILE", "LANDLINE").
    #[serde(rename = "type")]
    pub type_: Option<String>,
    /// Whether the phone is valid.
    pub is_valid: Option<bool>,
    /// Ranking of the phone.
    pub ranking: i32,
    /// Quality score of the data.
    pub quality_score: Option<f64>,
    /// Creation timestamp.
    pub created_at: String,
    /// Update timestamp.
    pub updated_at: String,
}

/// Address details from lookup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupAddress {
    /// Unique identifier.
    pub id: String,
    /// Street name.
    pub street: String,
    /// Street number.
    pub number: String,
    /// Complement (apartment, suite, etc.).
    pub complement: Option<String>,
    /// Neighborhood.
    pub neighborhood: String,
    /// City.
    pub city: String,
    /// State.
    pub state: String,
    /// Postal code (CEP).
    pub cep: String,
    /// Type of street.
    pub street_type: String,
    /// Latitude coordinate.
    pub latitude: f64,
    /// Longitude coordinate.
    pub longitude: f64,
    /// Ranking of the address.
    pub ranking: i32,
    /// Quality score of the address.
    pub quality_score: Option<f64>,
    /// Whether the address is valid.
    pub is_valid: Option<bool>,
    /// Creation timestamp.
    pub created_at: String,
    /// Update timestamp.
    pub updated_at: String,
}

/// Financial information from lookup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupFinancialInfo {
    /// Estimated income.
    pub income: Option<f32>,
    /// Income range description.
    pub income_range: Option<String>,
    /// Purchasing power indicators.
    pub purchasing_power: LookupPurchasingPower,
    /// Credit score information.
    pub credit_score: LookupCreditScore,
}

/// Purchasing power details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupPurchasingPower {
    /// Purchasing power code.
    pub code: Option<i32>,
    /// Estimated income for purchasing power.
    pub income: Option<f32>,
}

/// Credit score details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupCreditScore {
    /// Numerical score.
    pub score: f64,
    /// Risk level description.
    pub risk_level: Option<String>,
}

/// Interests and behavioral profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupInterests {
    /// Whether identified as middle class.
    pub middle_class: bool,
    /// Whether has accumulated miles.
    pub has_accumulated_miles: bool,
    /// Online shopping score.
    pub online_shopping: f64,
    /// Car insurance score.
    pub car_insurance: f64,
    /// Fitness interest score.
    pub fitness: f64,
    /// Customer ID associated with interests.
    pub customer_id: String,
    /// Whether owns luxury goods.
    pub owns_luxury_goods: bool,
    /// Whether owns a home.
    pub owns_home: bool,
    /// Multiple credit card score.
    pub multiple_credit_card: f64,
    /// Health insurance score.
    pub health_insurance: f64,
    /// Travel interest score.
    pub travel: f64,
    /// Creation timestamp.
    pub created_at: String,
    /// Whether owns investments.
    pub owns_investments: bool,
    /// Whether owns current accounts.
    pub owns_current_accounts: bool,
    /// Prime credit card score.
    pub prime_credit_card: f64,
    /// Life insurance score.
    pub life_insurance: f64,
    /// Luxury interest score.
    pub luxury: f64,
    /// Update timestamp.
    pub updated_at: String,
    /// Whether owns premium bank account.
    pub owns_premium_bank_account: bool,
    /// Whether owns car insurance.
    pub owns_car_insurance: bool,
    /// Cable TV score.
    pub cable_tv: f64,
    /// Home insurance score.
    pub home_insurance: f64,
    /// Moviegoer score.
    pub moviegoer: f64,
    /// Whether has pre-approved personal loan.
    pub pre_approved_personal_loan: bool,
    /// Whether owns a credit card.
    pub owns_credit_card: bool,
    /// Whether has a private retirement plan.
    pub has_private_retirement_plan: bool,
    /// Broadband internet score.
    pub broadband_internet: f64,
    /// Investments score.
    pub investments: f64,
    /// Public transportation score.
    pub public_transportation: f64,
    /// Unique identifier.
    pub id: String,
    /// Whether owns multiple credit cards.
    pub owns_multiple_credit_cards: bool,
    /// Personal loan score.
    pub personal_loan: f64,
    /// Own home score.
    pub own_home: f64,
    /// Consignment loan score.
    pub consignment_loan: f64,
    /// Online games score.
    pub online_games: f64,
    /// Whether has pre-approved mortgage.
    pub pre_approved_mortgage: bool,
    /// Whether owns a black credit card.
    pub owns_black_credit_card: bool,
    /// Vehicle loan score.
    pub vehicle_loan: f64,
    /// Private retirement plan score.
    pub private_retirement_plan: f64,
    /// Frequent flyer miles redemption score.
    pub frequent_flyer_miles_redemption: f64,
    /// Video games score.
    pub video_games: f64,
    /// Whether has pre-approved vehicle financing.
    pub pre_approved_vehicle_financing: bool,
    /// Whether owns a prime credit card.
    pub owns_prime_credit_card: bool,
    /// Mortgage score.
    pub mortgage: f64,
    /// Discount hunting score.
    pub discount_hunting: f64,
    /// Early adopter score.
    pub early_adopter: f64,
}

/// Education history details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupEducation {
    /// Unique identifier.
    pub id: String,
    /// Education level or institution.
    pub education: String,
    /// Customer ID.
    pub customer_id: String,
    /// Creation timestamp.
    pub created_at: String,
    /// Update timestamp.
    pub updated_at: String,
}

// ============ Enriched Customer Data ============

/// Aggregated enriched customer data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichedCustomerData {
    /// Core customer data.
    pub customer: Customer,
    /// List of emails.
    pub emails: Vec<Email>,
    /// List of phones.
    pub phones: Vec<Phone>,
    /// Full enrichment data from external sources.
    pub enrichment_data: Option<LookupResponse>,
}

// ============ Query Parameters ============

/// Query parameters for customer search.
#[derive(Debug, Deserialize)]
pub struct CustomerQueryParams {
    /// Filter by name.
    pub name: Option<String>,
    /// Filter by phone.
    pub phone: Option<String>,
    /// Filter by email.
    pub email: Option<String>,
    /// Filter by CPF.
    pub cpf: Option<String>,
}

// ============ Work API Models ============

// When querying modulo=cpf, Work API returns data directly at root level
/// Type alias for the complete Work API response.
pub type WorkApiCompleteResponse = serde_json::Value;

/// Represents a specific module in the Work API response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkApiModule {
    /// Status of the module response.
    pub status: String,
    /// Data content of the module.
    pub data: Option<serde_json::Value>,
}

// ============ Wealth Assessment (Summarized) ============

/// Summary of wealth and risk assessment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WealthAssessment {
    /// CPF document.
    pub cpf: String,
    /// Name.
    pub nome: String,
    /// Income description.
    pub renda: Option<String>,
    /// Purchasing power details.
    pub poder_aquisitivo: Option<PoderAquisitivo>,
    /// Credit score details.
    pub score: Option<ScoreInfo>,
    /// Mosaic segmentation details.
    pub mosaic: Option<MosaicInfo>,
    /// Related companies.
    pub empresas: Vec<EmpresaInfo>,
    /// Consumption profile summary.
    pub perfil_consumo: Option<PerfilConsumoSumario>,
    /// Recent purchases summary.
    pub compras_recentes: Option<ComprasSumario>,
    /// Wealth level assessment.
    pub assessment: WealthLevel,
}

/// Purchasing power details in Portuguese.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoderAquisitivo {
    /// Code.
    pub codigo: String,
    /// Description.
    pub descricao: String,
    /// Income value.
    pub renda: String,
    /// Income range.
    pub faixa: String,
}

/// Score information in Portuguese.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreInfo {
    /// CSB Score.
    pub score_csb: String,
    /// CSB risk range.
    pub faixa_risco_csb: String,
    /// CSBA Score.
    pub score_csba: String,
    /// CSBA risk range.
    pub faixa_risco_csba: String,
}

/// Mosaic segmentation information in Portuguese.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MosaicInfo {
    /// New code.
    pub codigo_novo: String,
    /// New description.
    pub descricao_novo: String,
    /// New class.
    pub classe_novo: String,
    /// Principal code.
    pub codigo_principal: String,
    /// Principal description.
    pub descricao_principal: String,
    /// Principal class.
    pub classe_principal: String,
}

/// Company relationship information in Portuguese.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmpresaInfo {
    /// CNPJ document.
    pub cnpj: String,
    /// Relationship type.
    pub tipo_relacao: String,
    /// Relationship description.
    pub relacao: String,
    /// Whether active.
    pub ativo: bool,
}

/// Consumption profile summary in Portuguese.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerfilConsumoSumario {
    /// Possesses luxury items.
    pub possui_luxo: bool,
    /// Has investments.
    pub possui_investimentos: bool,
    /// Has Black credit card.
    pub possui_cartao_black: bool,
    /// Has Prime credit card.
    pub possui_cartao_prime: bool,
    /// Has high standard account.
    pub possui_conta_alto_padrao: bool,
    /// Owns home.
    pub possui_casa_propria: bool,
    /// Has private pension.
    pub possui_previdencia_privada: bool,
    /// Pre-approved credit details.
    pub credito_pre_aprovado: CreditoPreAprovado,
    /// Key probabilities.
    pub probabilidades_chave: ProbabilidadesChave,
}

/// Pre-approved credit details in Portuguese.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditoPreAprovado {
    /// Personal loan.
    pub pessoal: bool,
    /// Real estate loan.
    pub imobiliario: bool,
    /// Vehicle loan.
    pub veiculo: bool,
}

/// Key probabilities in Portuguese.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbabilidadesChave {
    /// Investments probability.
    pub investimentos: String,
    /// Luxury probability.
    pub luxo: String,
    /// Tourism probability.
    pub turismo: String,
    /// Early adopter probability.
    pub early_adopters: String,
}

/// Recent purchases summary in Portuguese.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComprasSumario {
    /// Total number of purchases.
    pub total_compras: usize,
    /// Total value.
    pub valor_total: f64,
    /// Average ticket value.
    pub ticket_medio: f64,
    /// List of luxury items.
    pub itens_luxo: Vec<String>,
}

/// Wealth level classification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WealthLevel {
    /// Very high wealth.
    #[serde(rename = "MUITO_ALTO")]
    MuitoAlto {
        /// Score.
        score: u32,
        /// Indicators.
        indicadores: Vec<String>,
    },
    /// High wealth.
    #[serde(rename = "ALTO")]
    Alto {
        /// Score.
        score: u32,
        /// Indicators.
        indicadores: Vec<String>,
    },
    /// Medium-high wealth.
    #[serde(rename = "MEDIO_ALTO")]
    MedioAlto {
        /// Score.
        score: u32,
        /// Indicators.
        indicadores: Vec<String>,
    },
    /// Medium wealth.
    #[serde(rename = "MEDIO")]
    Medio {
        /// Score.
        score: u32,
        /// Indicators.
        indicadores: Vec<String>,
    },
    /// Low wealth.
    #[serde(rename = "BAIXO")]
    Baixo {
        /// Score.
        score: u32,
        /// Indicators.
        indicadores: Vec<String>,
    },
}

// ============ Unified Customer Response for C2S ============

/// Unified customer response format for C2S integration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedCustomerResponse {
    /// Data source.
    pub source: String,
    /// Entity type.
    #[serde(rename = "type")]
    pub type_: String,
    /// Personal info.
    pub personal_info: UnifiedPersonalInfo,
    /// Contact info.
    pub contact_info: UnifiedContactInfo,
    /// Addresses.
    pub addresses: Vec<UnifiedAddress>,
    /// Financial info.
    pub financial_info: Option<UnifiedFinancialInfo>,
    /// Interests (dynamic).
    pub interests: Option<serde_json::Value>,
    /// Metadata about the response.
    pub metadata: ResponseMetadata,
    /// Wealth assessment.
    pub wealth_assessment: Option<WealthAssessment>,
}

/// Unified personal info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedPersonalInfo {
    /// CPF.
    pub cpf: Option<String>,
    /// Name.
    pub name: Option<String>,
    /// Birth date.
    pub birth_date: Option<String>,
    /// Gender.
    pub gender: Option<String>,
    /// Mother's name.
    pub mother_name: Option<String>,
    /// Father's name.
    pub father_name: Option<String>,
    /// Marital status.
    pub marital_status: Option<String>,
    /// RG.
    pub rg: Option<String>,
    /// Voter ID.
    pub voter_id: Option<String>,
}

/// Unified contact info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedContactInfo {
    /// Emails.
    pub emails: Vec<UnifiedEmail>,
    /// Phones.
    pub phones: Vec<UnifiedPhone>,
}

/// Unified email info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedEmail {
    /// Email address.
    pub email: String,
    /// Is valid.
    pub is_valid: Option<bool>,
    /// Source.
    pub source: String,
}

/// Unified phone info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedPhone {
    /// Phone number.
    pub phone: String,
    /// DDD.
    pub ddd: Option<String>,
    /// Operator.
    pub operator: Option<String>,
    /// Type.
    #[serde(rename = "type")]
    pub type_: Option<String>,
    /// Is valid.
    pub is_valid: Option<bool>,
    /// Source.
    pub source: String,
}

/// Unified address info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedAddress {
    /// Street.
    pub street: Option<String>,
    /// Number.
    pub number: Option<String>,
    /// Complement.
    pub complement: Option<String>,
    /// Neighborhood.
    pub neighborhood: Option<String>,
    /// City.
    pub city: Option<String>,
    /// State.
    pub state: Option<String>,
    /// CEP.
    pub cep: Option<String>,
    /// Source.
    pub source: String,
}

/// Unified financial info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedFinancialInfo {
    /// Income.
    pub income: Option<f32>,
    /// Income range.
    pub income_range: Option<String>,
    /// Credit score.
    pub credit_score: Option<f64>,
}

/// Response metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMetadata {
    /// Whether data was enriched.
    pub enriched: bool,
    /// Sources consulted.
    pub sources: Vec<String>,
    /// Timestamp.
    pub timestamp: String,
    /// Modules consulted.
    pub modules_consulted: Vec<String>,
}
