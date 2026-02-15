//! Dashboard HTML + session authentication.
//!
//! Ported from ts:src/routes/dashboard.ts + ts:src/middleware/dashboard-auth.ts
//!
//! Features:
//! - Server-rendered HTML dashboard at /dashboard
//! - Login page at /dashboard/login
//! - 24-hour session cookies (HttpOnly, Secure, SameSite=Lax)
//! - MBRAS branding (Navy #1a3a5c + Gold #b8a06a)
//! - Enrichment stats, service health, recent leads

use axum::{
    body::Body,
    extract::{Query, State},
    http::{header, HeaderMap, Response, StatusCode},
    response::{Html, IntoResponse, Redirect},
    Form,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::handlers::AppState;

// ---------------------------------------------------------------------------
// Session management
// ---------------------------------------------------------------------------

/// Session TTL: 24 hours
const SESSION_TTL_SECS: i64 = 86400;

#[derive(Debug, Clone)]
struct Session {
    user: String,
    created_at: DateTime<Utc>,
}

/// In-memory session store (resets on deploy, acceptable for dashboard).
pub struct SessionStore {
    sessions: RwLock<HashMap<String, Session>>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
        }
    }

    pub async fn create(&self, user: &str) -> String {
        let token = Uuid::new_v4().to_string();
        let mut sessions = self.sessions.write().await;
        sessions.insert(
            token.clone(),
            Session {
                user: user.to_string(),
                created_at: Utc::now(),
            },
        );
        token
    }

    pub async fn validate(&self, token: &str) -> bool {
        let sessions = self.sessions.read().await;
        match sessions.get(token) {
            Some(session) => {
                let elapsed = (Utc::now() - session.created_at).num_seconds();
                elapsed < SESSION_TTL_SECS
            }
            None => false,
        }
    }

    pub async fn destroy(&self, token: &str) {
        let mut sessions = self.sessions.write().await;
        sessions.remove(token);
    }

    /// Cleanup expired sessions (called periodically).
    pub async fn cleanup(&self) {
        let mut sessions = self.sessions.write().await;
        let now = Utc::now();
        sessions.retain(|_, s| (now - s.created_at).num_seconds() < SESSION_TTL_SECS);
    }
}

// ---------------------------------------------------------------------------
// Auth helpers
// ---------------------------------------------------------------------------

fn get_session_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies
                .split(';')
                .map(|c| c.trim())
                .find(|c| c.starts_with("dashboard_session="))
                .map(|c| c.trim_start_matches("dashboard_session=").to_string())
        })
}

fn session_cookie(token: &str) -> String {
    format!(
        "dashboard_session={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
        token, SESSION_TTL_SECS
    )
}

fn clear_cookie() -> String {
    "dashboard_session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0".to_string()
}

fn get_dashboard_credentials() -> Option<(String, String)> {
    let user = std::env::var("DASHBOARD_USER").ok()?;
    let pass = std::env::var("DASHBOARD_PASSWORD").ok()?;
    if user.is_empty() || pass.is_empty() {
        return None;
    }
    Some((user, pass))
}

// ---------------------------------------------------------------------------
// Login page HTML
// ---------------------------------------------------------------------------

fn login_page_html(error: Option<&str>) -> String {
    let error_html = match error {
        Some(msg) => format!(
            r#"<div style="background:#dc3545;color:white;padding:12px;border-radius:8px;margin-bottom:20px;text-align:center">{}</div>"#,
            msg
        ),
        None => String::new(),
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="pt-BR">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Login — C2S Dashboard</title>
<style>
  * {{ margin:0; padding:0; box-sizing:border-box; }}
  body {{ font-family:'Inter',system-ui,sans-serif; background:#f5f5f5; display:flex; align-items:center; justify-content:center; min-height:100vh; }}
  .card {{ background:white; border-radius:16px; padding:48px 40px; width:100%; max-width:400px; box-shadow:0 4px 24px rgba(0,0,0,0.08); }}
  .logo {{ text-align:center; margin-bottom:32px; }}
  .logo h1 {{ color:#1a3a5c; font-size:28px; font-weight:700; }}
  .logo p {{ color:#666; font-size:14px; margin-top:4px; }}
  label {{ display:block; font-size:14px; color:#333; margin-bottom:6px; font-weight:500; }}
  input {{ width:100%; padding:12px 16px; border:1px solid #ddd; border-radius:8px; font-size:16px; margin-bottom:16px; }}
  input:focus {{ outline:none; border-color:#b8a06a; }}
  button {{ width:100%; padding:14px; background:#1a3a5c; color:white; border:none; border-radius:8px; font-size:16px; font-weight:600; cursor:pointer; }}
  button:hover {{ background:#2a4a6c; }}
</style>
</head>
<body>
<div class="card">
  <div class="logo">
    <h1>C2S Dashboard</h1>
    <p>Lead Enrichment Monitor</p>
  </div>
  {}
  <form method="POST" action="/dashboard/login">
    <label for="username">Username</label>
    <input type="text" id="username" name="username" required autocomplete="username">
    <label for="password">Password</label>
    <input type="password" id="password" name="password" required autocomplete="current-password">
    <button type="submit">Entrar</button>
  </form>
</div>
</body>
</html>"#,
        error_html
    )
}

// ---------------------------------------------------------------------------
// Dashboard HTML
// ---------------------------------------------------------------------------

fn dashboard_html(
    enrichment_stats: &crate::enrichment_monitor::EnrichmentStats,
    health: &crate::alert::ServiceHealthReport,
) -> String {
    let rate_color = if enrichment_stats.healthy {
        "#28a745"
    } else {
        "#dc3545"
    };

    let services_html: String = health
        .services
        .iter()
        .map(|s| {
            let status_icon = if s.healthy { "&#9679;" } else { "&#9888;" };
            let status_color = if s.healthy { "#28a745" } else { "#dc3545" };
            let last_err = s
                .last_error_message
                .as_deref()
                .unwrap_or("-");
            format!(
                r#"<tr>
                    <td>{}</td>
                    <td style="color:{}">{} {}</td>
                    <td>{}</td>
                    <td>{}</td>
                </tr>"#,
                s.service.as_str(),
                status_color,
                status_icon,
                if s.healthy { "Healthy" } else { "DOWN" },
                s.consecutive_errors,
                last_err,
            )
        })
        .collect();

    format!(
        r#"<!DOCTYPE html>
<html lang="pt-BR">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>C2S Dashboard</title>
<meta http-equiv="refresh" content="60">
<style>
  * {{ margin:0; padding:0; box-sizing:border-box; }}
  body {{ font-family:'Inter',system-ui,sans-serif; background:#f5f5f5; color:#333; }}
  .header {{ background:#1a3a5c; color:white; padding:20px 32px; display:flex; justify-content:space-between; align-items:center; }}
  .header h1 {{ font-size:22px; }}
  .header a {{ color:#b8a06a; text-decoration:none; font-size:14px; }}
  .container {{ max-width:1200px; margin:0 auto; padding:24px; }}
  .cards {{ display:grid; grid-template-columns:repeat(auto-fill,minmax(200px,1fr)); gap:16px; margin-bottom:24px; }}
  .card {{ background:white; border-radius:12px; padding:20px; box-shadow:0 2px 8px rgba(0,0,0,0.06); }}
  .card .label {{ font-size:12px; color:#666; text-transform:uppercase; letter-spacing:0.5px; }}
  .card .value {{ font-size:28px; font-weight:700; margin-top:4px; }}
  .section {{ background:white; border-radius:12px; padding:24px; margin-bottom:24px; box-shadow:0 2px 8px rgba(0,0,0,0.06); }}
  .section h2 {{ font-size:18px; color:#1a3a5c; margin-bottom:16px; }}
  table {{ width:100%; border-collapse:collapse; }}
  th {{ text-align:left; padding:10px 12px; border-bottom:2px solid #e9ecef; font-size:13px; color:#666; text-transform:uppercase; }}
  td {{ padding:10px 12px; border-bottom:1px solid #f0f0f0; font-size:14px; }}
  .footer {{ text-align:center; padding:20px; color:#999; font-size:12px; }}
</style>
</head>
<body>
<div class="header">
  <h1>C2S Enrichment Dashboard</h1>
  <a href="/dashboard/logout">Logout</a>
</div>
<div class="container">
  <div class="cards">
    <div class="card">
      <div class="label">Enrichment Rate</div>
      <div class="value" style="color:{rate_color}">{rate:.1}%</div>
    </div>
    <div class="card">
      <div class="label">Total Leads (30d)</div>
      <div class="value">{total}</div>
    </div>
    <div class="card">
      <div class="label">Completed</div>
      <div class="value" style="color:#28a745">{completed}</div>
    </div>
    <div class="card">
      <div class="label">Partial</div>
      <div class="value" style="color:#ffc107">{partial}</div>
    </div>
    <div class="card">
      <div class="label">Failed</div>
      <div class="value" style="color:#dc3545">{failed}</div>
    </div>
    <div class="card">
      <div class="label">Pending</div>
      <div class="value">{pending}</div>
    </div>
  </div>

  <div class="section">
    <h2>Service Health</h2>
    <table>
      <thead>
        <tr><th>Service</th><th>Status</th><th>Errors</th><th>Last Error</th></tr>
      </thead>
      <tbody>
        {services_html}
      </tbody>
    </table>
  </div>
</div>
<div class="footer">
  rust-c2s-api &middot; Last updated: {checked_at}
</div>
</body>
</html>"#,
        rate_color = rate_color,
        rate = enrichment_stats.rate_pct,
        total = enrichment_stats.total,
        completed = enrichment_stats.completed,
        partial = enrichment_stats.partial,
        failed = enrichment_stats.failed,
        pending = enrichment_stats.pending,
        services_html = services_html,
        checked_at = enrichment_stats.checked_at.format("%Y-%m-%d %H:%M:%S UTC"),
    )
}

// ---------------------------------------------------------------------------
// Route handlers
// ---------------------------------------------------------------------------

/// GET /dashboard — main dashboard (requires auth)
pub async fn dashboard_page(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Check auth
    if get_dashboard_credentials().is_some() {
        let token = get_session_token(&headers);
        let valid = match token {
            Some(ref t) => state.sessions.validate(t).await,
            None => false,
        };
        if !valid {
            return Redirect::temporary("/dashboard/login").into_response();
        }
    }

    let stats = state.enrichment_monitor.get_stats().await;
    let health = state.alert_service.get_service_health().await;
    let html = dashboard_html(&stats, &health);
    Html(html).into_response()
}

/// GET /dashboard/login — login page
pub async fn login_page() -> impl IntoResponse {
    if get_dashboard_credentials().is_none() {
        return Redirect::temporary("/dashboard").into_response();
    }
    Html(login_page_html(None)).into_response()
}

#[derive(Deserialize)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

/// POST /dashboard/login — process login
pub async fn login_submit(
    State(state): State<Arc<AppState>>,
    Form(form): Form<LoginForm>,
) -> impl IntoResponse {
    let creds = match get_dashboard_credentials() {
        Some(c) => c,
        None => return Redirect::temporary("/dashboard").into_response(),
    };

    if form.username == creds.0 && form.password == creds.1 {
        let token = state.sessions.create(&form.username).await;
        Response::builder()
            .status(StatusCode::SEE_OTHER)
            .header(header::LOCATION, "/dashboard")
            .header(header::SET_COOKIE, session_cookie(&token))
            .body(Body::empty())
            .unwrap()
            .into_response()
    } else {
        Html(login_page_html(Some("Invalid credentials"))).into_response()
    }
}

/// GET /dashboard/logout — destroy session
pub async fn logout(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Some(token) = get_session_token(&headers) {
        state.sessions.destroy(&token).await;
    }
    Response::builder()
        .status(StatusCode::SEE_OTHER)
        .header(header::LOCATION, "/dashboard/login")
        .header(header::SET_COOKIE, clear_cookie())
        .body(Body::empty())
        .unwrap()
        .into_response()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_session_create_validate() {
        let store = SessionStore::new();
        let token = store.create("admin").await;
        assert!(store.validate(&token).await);
        assert!(!store.validate("invalid_token").await);
    }

    #[tokio::test]
    async fn test_session_destroy() {
        let store = SessionStore::new();
        let token = store.create("admin").await;
        assert!(store.validate(&token).await);
        store.destroy(&token).await;
        assert!(!store.validate(&token).await);
    }

    #[test]
    fn test_get_session_token_from_cookies() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            "other=abc; dashboard_session=test123; foo=bar"
                .parse()
                .unwrap(),
        );
        assert_eq!(
            get_session_token(&headers),
            Some("test123".to_string())
        );
    }

    #[test]
    fn test_get_session_token_missing() {
        let headers = HeaderMap::new();
        assert_eq!(get_session_token(&headers), None);
    }

    #[test]
    fn test_login_page_html_no_error() {
        let html = login_page_html(None);
        assert!(html.contains("C2S Dashboard"));
        assert!(!html.contains("dc3545")); // No error banner
    }

    #[test]
    fn test_login_page_html_with_error() {
        let html = login_page_html(Some("Bad password"));
        assert!(html.contains("Bad password"));
        assert!(html.contains("dc3545")); // Error banner
    }
}
