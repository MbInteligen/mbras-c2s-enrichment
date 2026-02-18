use crate::errors::AppError;
use crate::handlers::AppState;
use crate::ibvi_property::IbviPropertyService;
use crate::services::WorkApiService;
use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct ContextQueryParams {
    pub cpf: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
}

/// GET /api/v1/context?cpf=...&phone=...&email=...
///
/// Unified context endpoint that fans out to Work API, Meilisearch (companies),
/// and IBVI (properties) in parallel via `tokio::join!`.
/// Returns a combined JSON object with data from all sources.
pub async fn get_context(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ContextQueryParams>,
) -> Result<Json<serde_json::Value>, AppError> {
    if params.cpf.is_none() && params.phone.is_none() && params.email.is_none() {
        return Err(AppError::BadRequest(
            "At least one of cpf, phone, or email is required".to_string(),
        ));
    }

    let cpf = resolve_cpf(&state, &params).await?;
    tracing::info!("GET /api/v1/context — resolved CPF: {}", cpf);

    // Fan out to all three sources in parallel
    let work_api = WorkApiService::new(&state.config);
    let db_clone = state.db.clone();
    let meilisearch = state.meilisearch.clone();
    let cpf_for_meili = cpf.clone();
    let cpf_for_ibvi = cpf.clone();

    let (work_result, companies, properties) = tokio::join!(
        work_api.fetch_all_modules(&cpf),
        meilisearch.find_companies_by_cpf(&cpf_for_meili),
        async move {
            IbviPropertyService::new(db_clone)
                .find_properties_by_cpf(&cpf_for_ibvi)
                .await
        }
    );

    let work_data = match work_result {
        Ok(data) => Some(data),
        Err(e) => {
            tracing::warn!("Work API failed for context CPF {}: {}", cpf, e);
            None
        }
    };

    let has_companies = companies.total_companies > 0;
    let has_properties = properties.is_some();

    Ok(Json(json!({
        "cpf": cpf,
        "work_api": work_data,
        "companies": companies,
        "properties": properties,
        "sources": {
            "work_api": work_data.is_some(),
            "meilisearch": has_companies,
            "ibvi": has_properties,
        }
    })))
}

/// Resolve a CPF from the query parameters.
/// If `cpf` is provided directly, normalize and validate it.
/// Otherwise, discover CPF from phone/email via the multi-tier fallback system.
async fn resolve_cpf(
    state: &Arc<AppState>,
    params: &ContextQueryParams,
) -> Result<String, AppError> {
    if let Some(ref cpf) = params.cpf {
        let normalized: String = cpf.chars().filter(|c| c.is_ascii_digit()).collect();
        if normalized.len() == 11 {
            return Ok(normalized);
        }
        return Err(AppError::BadRequest(format!(
            "Invalid CPF: expected 11 digits, got {}",
            normalized.len()
        )));
    }

    let phone = params.phone.as_deref();
    let email = params.email.as_deref();

    let result =
        crate::enrichment::find_cpf_via_diretrix(phone, email, &state.config, None).await?;

    result
        .cpfs
        .into_iter()
        .next()
        .ok_or_else(|| AppError::NotFound("CPF not found for given phone/email".to_string()))
}
