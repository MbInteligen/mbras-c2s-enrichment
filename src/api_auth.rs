//! API key authentication middleware.
//!
//! Ported from ts:src/middleware/auth.ts
//!
//! Validates Bearer token or X-API-Key header against WORKER_API_KEY.
//! Skips auth for health, metrics, dashboard, and webhook endpoints.

use axum::{
    body::Body,
    extract::Request,
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde_json::json;

/// Paths that bypass API key auth.
const PUBLIC_PATHS: &[&str] = &[
    "/health",
    "/metrics",
    "/dashboard",
    "/docs",
    "/api-docs",
];

/// Paths that use their own auth (webhooks use webhook_secret).
const WEBHOOK_PATHS: &[&str] = &[
    "/api/v1/webhooks/",
];

/// API key auth middleware for Axum.
pub async fn api_key_auth(req: Request<Body>, next: Next) -> Response {
    let path = req.uri().path().to_string();

    // Skip auth for public paths
    for public in PUBLIC_PATHS {
        if path.starts_with(public) {
            return next.run(req).await;
        }
    }

    // Skip auth for webhook paths (they have their own auth)
    for webhook in WEBHOOK_PATHS {
        if path.starts_with(webhook) {
            return next.run(req).await;
        }
    }

    // Get expected API key
    let expected_key = match std::env::var("WORKER_API_KEY").ok().filter(|s| !s.is_empty()) {
        Some(k) => k,
        None => {
            // No API key configured — allow all requests (development mode)
            return next.run(req).await;
        }
    };

    // Extract key from Authorization header or X-API-Key
    let provided_key = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string())
        .or_else(|| {
            req.headers()
                .get("X-API-Key")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        });

    match provided_key {
        Some(key) if key == expected_key => next.run(req).await,
        Some(_) => (
            StatusCode::FORBIDDEN,
            axum::Json(json!({ "error": "Invalid API key" })),
        )
            .into_response(),
        None => (
            StatusCode::UNAUTHORIZED,
            axum::Json(json!({ "error": "API key required. Use Authorization: Bearer <key> or X-API-Key header" })),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_public_paths() {
        let public = vec!["/health", "/metrics", "/dashboard", "/dashboard/login", "/docs"];
        for path in public {
            assert!(
                PUBLIC_PATHS.iter().any(|p| path.starts_with(p)),
                "{} should be public",
                path
            );
        }
    }

    #[test]
    fn test_webhook_paths() {
        assert!(WEBHOOK_PATHS
            .iter()
            .any(|p| "/api/v1/webhooks/c2s".starts_with(p)));
        assert!(WEBHOOK_PATHS
            .iter()
            .any(|p| "/api/v1/webhooks/google-ads".starts_with(p)));
    }
}
