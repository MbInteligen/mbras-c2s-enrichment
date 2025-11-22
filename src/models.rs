use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// ============ Database Models ============

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Party {
    pub id: Uuid,
    pub party_type: String,
    pub cpf_cnpj: String,
    pub full_name: String,
    pub normalized_name: Option<String>,
    pub sex: Option<String>,
    pub birth_date: Option<NaiveDate>,
    pub mother_name: Option<String>,
    pub father_name: Option<String>,
    pub rg: Option<String>,
    pub fantasy_name: Option<String>,
    pub normalized_fantasy_name: Option<String>,
    pub opening_date: Option<NaiveDate>,
    pub registration_status_date: Option<NaiveDate>,
    pub company_type: Option<String>,
    pub company_size: Option<String>,
    pub enriched: Option<bool>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

// Alias for backward compatibility
pub type Customer = Party;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Person {
    pub party_id: Uuid,
    pub full_name: String,
    pub mothers_name: Option<String>,
    pub birth_date: Option<NaiveDate>,
    pub sex: Option<String>,
    pub marital_status: Option<String>,
    pub document_cpf: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Company {
    pub party_id: Uuid,
    pub legal_name: String,
    pub trade_name: Option<String>,
    pub cnpj: Option<String>,
    pub company_size: Option<String>,
    pub industry: Option<String>,
    pub foundation_date: Option<NaiveDate>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct PartyContact {
    pub contact_id: Uuid,
    pub party_id: Uuid,
    pub contact_type: String,
    pub value: String,
    pub is_primary: bool,
    pub is_verified: bool,
    pub is_whatsapp: bool,
    pub source: Option<String>,
    pub confidence: Option<f64>,
    pub valid_from: Option<DateTime<Utc>>,
    pub valid_to: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Email {
    pub id: Uuid,
    pub email: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Phone {
    pub id: Uuid,
    pub number: String,
    pub country_code: Option<String>,
    pub created_at: DateTime<Utc>,
}

// ============ API Request/Response Models ============

#[derive(Debug, Deserialize)]
pub struct LeadRequest {
    pub lead_id: String,
    pub personal_info: PersonalInfo,
    pub contact_info: ContactInfo,
}

#[derive(Debug, Deserialize)]
pub struct PersonalInfo {
    pub name: String,
    pub email: Option<String>,
    pub cpf: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ContactInfo {
    pub phones: Vec<PhoneInfo>,
}

#[derive(Debug, Deserialize)]
pub struct PhoneInfo {
    pub phone: String,
}

#[derive(Debug, Serialize)]
pub struct LeadResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<EnrichedCustomerData>,
}

// ============ Lookup Response (matches Go LookupResponse) ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupResponse {
    pub source: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub personal_info: LookupPersonalInfo,
    pub contact_info: LookupContactInfo,
    pub addresses: Vec<LookupAddress>,
    pub financial_info: LookupFinancialInfo,
    pub jobs: Vec<serde_json::Value>,
    pub vehicles: Vec<serde_json::Value>,
    pub interests: LookupInterests,
    pub purchase_history: Option<serde_json::Value>,
    pub educations: Vec<LookupEducation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupPersonalInfo {
    pub cpf: String,
    pub name: String,
    pub birth_date: Option<String>,
    pub gender: Option<String>,
    pub mother_name: Option<String>,
    pub father_name: Option<String>,
    pub marital_status: Option<String>,
    pub nationality: Option<String>,
    pub rg: Option<String>,
    pub voter_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupContactInfo {
    pub emails: Vec<LookupEmail>,
    pub phones: Vec<LookupPhone>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupEmail {
    pub id: String,
    pub email: String,
    pub is_valid: bool,
    pub ranking: i32,
    pub quality_score: f64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupPhone {
    pub id: String,
    pub phone: String,
    pub ddd: String,
    pub operator: Option<String>,
    #[serde(rename = "type")]
    pub type_: Option<String>,
    pub is_valid: Option<bool>,
    pub ranking: i32,
    pub quality_score: Option<f64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupAddress {
    pub id: String,
    pub street: String,
    pub number: String,
    pub complement: Option<String>,
    pub neighborhood: String,
    pub city: String,
    pub state: String,
    pub cep: String,
    pub street_type: String,
    pub latitude: f64,
    pub longitude: f64,
    pub ranking: i32,
    pub quality_score: Option<f64>,
    pub is_valid: Option<bool>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupFinancialInfo {
    pub income: Option<f32>,
    pub income_range: Option<String>,
    pub purchasing_power: LookupPurchasingPower,
    pub credit_score: LookupCreditScore,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupPurchasingPower {
    pub code: Option<i32>,
    pub income: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupCreditScore {
    pub score: f64,
    pub risk_level: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupInterests {
    pub middle_class: bool,
    pub has_accumulated_miles: bool,
    pub online_shopping: f64,
    pub car_insurance: f64,
    pub fitness: f64,
    pub customer_id: String,
    pub owns_luxury_goods: bool,
    pub owns_home: bool,
    pub multiple_credit_card: f64,
    pub health_insurance: f64,
    pub travel: f64,
    pub created_at: String,
    pub owns_investments: bool,
    pub owns_current_accounts: bool,
    pub prime_credit_card: f64,
    pub life_insurance: f64,
    pub luxury: f64,
    pub updated_at: String,
    pub owns_premium_bank_account: bool,
    pub owns_car_insurance: bool,
    pub cable_tv: f64,
    pub home_insurance: f64,
    pub moviegoer: f64,
    pub pre_approved_personal_loan: bool,
    pub owns_credit_card: bool,
    pub has_private_retirement_plan: bool,
    pub broadband_internet: f64,
    pub investments: f64,
    pub public_transportation: f64,
    pub id: String,
    pub owns_multiple_credit_cards: bool,
    pub personal_loan: f64,
    pub own_home: f64,
    pub consignment_loan: f64,
    pub online_games: f64,
    pub pre_approved_mortgage: bool,
    pub owns_black_credit_card: bool,
    pub vehicle_loan: f64,
    pub private_retirement_plan: f64,
    pub frequent_flyer_miles_redemption: f64,
    pub video_games: f64,
    pub pre_approved_vehicle_financing: bool,
    pub owns_prime_credit_card: bool,
    pub mortgage: f64,
    pub discount_hunting: f64,
    pub early_adopter: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupEducation {
    pub id: String,
    pub education: String,
    pub customer_id: String,
    pub created_at: String,
    pub updated_at: String,
}

// ============ Enriched Customer Data ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichedCustomerData {
    pub customer: Customer,
    pub emails: Vec<Email>,
    pub phones: Vec<Phone>,
    pub enrichment_data: Option<LookupResponse>,
}

// ============ Query Parameters ============

#[derive(Debug, Deserialize)]
pub struct CustomerQueryParams {
    pub name: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub cpf: Option<String>,
}

// ============ Work API Models ============

// When querying modulo=cpf, Work API returns data directly at root level
pub type WorkApiCompleteResponse = serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkApiModule {
    pub status: String,
    pub data: Option<serde_json::Value>,
}

// ============ Wealth Assessment (Summarized) ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WealthAssessment {
    pub cpf: String,
    pub nome: String,
    pub renda: Option<String>,
    pub poder_aquisitivo: Option<PoderAquisitivo>,
    pub score: Option<ScoreInfo>,
    pub mosaic: Option<MosaicInfo>,
    pub empresas: Vec<EmpresaInfo>,
    pub perfil_consumo: Option<PerfilConsumoSumario>,
    pub compras_recentes: Option<ComprasSumario>,
    pub assessment: WealthLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoderAquisitivo {
    pub codigo: String,
    pub descricao: String,
    pub renda: String,
    pub faixa: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreInfo {
    pub score_csb: String,
    pub faixa_risco_csb: String,
    pub score_csba: String,
    pub faixa_risco_csba: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MosaicInfo {
    pub codigo_novo: String,
    pub descricao_novo: String,
    pub classe_novo: String,
    pub codigo_principal: String,
    pub descricao_principal: String,
    pub classe_principal: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmpresaInfo {
    pub cnpj: String,
    pub tipo_relacao: String,
    pub relacao: String,
    pub ativo: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerfilConsumoSumario {
    pub possui_luxo: bool,
    pub possui_investimentos: bool,
    pub possui_cartao_black: bool,
    pub possui_cartao_prime: bool,
    pub possui_conta_alto_padrao: bool,
    pub possui_casa_propria: bool,
    pub possui_previdencia_privada: bool,
    pub credito_pre_aprovado: CreditoPreAprovado,
    pub probabilidades_chave: ProbabilidadesChave,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditoPreAprovado {
    pub pessoal: bool,
    pub imobiliario: bool,
    pub veiculo: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbabilidadesChave {
    pub investimentos: String,
    pub luxo: String,
    pub turismo: String,
    pub early_adopters: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComprasSumario {
    pub total_compras: usize,
    pub valor_total: f64,
    pub ticket_medio: f64,
    pub itens_luxo: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WealthLevel {
    #[serde(rename = "MUITO_ALTO")]
    MuitoAlto {
        score: u32,
        indicadores: Vec<String>,
    },
    #[serde(rename = "ALTO")]
    Alto {
        score: u32,
        indicadores: Vec<String>,
    },
    #[serde(rename = "MEDIO_ALTO")]
    MedioAlto {
        score: u32,
        indicadores: Vec<String>,
    },
    #[serde(rename = "MEDIO")]
    Medio {
        score: u32,
        indicadores: Vec<String>,
    },
    #[serde(rename = "BAIXO")]
    Baixo {
        score: u32,
        indicadores: Vec<String>,
    },
}

// ============ Unified Customer Response for C2S ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedCustomerResponse {
    pub source: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub personal_info: UnifiedPersonalInfo,
    pub contact_info: UnifiedContactInfo,
    pub addresses: Vec<UnifiedAddress>,
    pub financial_info: Option<UnifiedFinancialInfo>,
    pub interests: Option<serde_json::Value>,
    pub metadata: ResponseMetadata,
    pub wealth_assessment: Option<WealthAssessment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedPersonalInfo {
    pub cpf: Option<String>,
    pub name: Option<String>,
    pub birth_date: Option<String>,
    pub gender: Option<String>,
    pub mother_name: Option<String>,
    pub father_name: Option<String>,
    pub marital_status: Option<String>,
    pub rg: Option<String>,
    pub voter_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedContactInfo {
    pub emails: Vec<UnifiedEmail>,
    pub phones: Vec<UnifiedPhone>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedEmail {
    pub email: String,
    pub is_valid: Option<bool>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedPhone {
    pub phone: String,
    pub ddd: Option<String>,
    pub operator: Option<String>,
    #[serde(rename = "type")]
    pub type_: Option<String>,
    pub is_valid: Option<bool>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedAddress {
    pub street: Option<String>,
    pub number: Option<String>,
    pub complement: Option<String>,
    pub neighborhood: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub cep: Option<String>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedFinancialInfo {
    pub income: Option<f32>,
    pub income_range: Option<String>,
    pub credit_score: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMetadata {
    pub enriched: bool,
    pub sources: Vec<String>,
    pub timestamp: String,
    pub modules_consulted: Vec<String>,
}
