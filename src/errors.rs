use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::fmt;

/// Application-specific error types.
#[derive(Debug)]
pub enum AppError {
    /// Database-related errors.
    DatabaseError(sqlx::Error),
    /// Resource not found error.
    NotFound(String),
    /// Bad request error (invalid input).
    BadRequest(String),
    /// Error interacting with an external API.
    ExternalApiError(String),
    /// Internal server error.
    InternalError(String),
    /// Unauthorized access error.
    Unauthorized(String),
    /// Error with context chain for better debugging.
    WithContext {
        /// The underlying source of the error.
        source: Box<AppError>,
        /// Additional context message.
        context: String,
    },
}

impl fmt::Display for AppError {
    /// Formats the error for display.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::DatabaseError(e) => write!(f, "Database error: {}", e),
            AppError::NotFound(msg) => write!(f, "Not found: {}", msg),
            AppError::BadRequest(msg) => write!(f, "Bad request: {}", msg),
            AppError::ExternalApiError(msg) => write!(f, "External API error: {}", msg),
            AppError::InternalError(msg) => write!(f, "Internal error: {}", msg),
            AppError::Unauthorized(msg) => write!(f, "Unauthorized: {}", msg),
            AppError::WithContext { source, context } => {
                write!(f, "{}: {}", context, source)
            }
        }
    }
}

impl IntoResponse for AppError {
    /// Converts the error into an HTTP response.
    ///
    /// Maps each error variant to an appropriate HTTP status code and JSON body.
    /// Logs errors appropriately based on their severity.
    fn into_response(self) -> Response {
        let (status, error_message) = match &self {
            AppError::DatabaseError(e) => {
                tracing::error!("Database error: {:?}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Database error".to_string(),
                )
            }
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::ExternalApiError(msg) => {
                tracing::error!("External API error: {}", msg);
                (
                    StatusCode::BAD_GATEWAY,
                    "External service error".to_string(),
                )
            }
            AppError::InternalError(msg) => {
                tracing::error!("Internal error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
            AppError::Unauthorized(msg) => {
                tracing::warn!("Unauthorized access: {}", msg);
                (StatusCode::UNAUTHORIZED, "Unauthorized".to_string())
            }
            AppError::WithContext { source, context } => {
                // Log full context chain for debugging
                tracing::error!("Error with context: {} -> {}", context, source);
                // Delegate to underlying error's response
                return source.clone().into_response();
            }
        };

        let body = Json(json!({
            "error": error_message,
        }));

        (status, body).into_response()
    }
}

// Make AppError cloneable for WithContext variant
impl Clone for AppError {
    /// Clones the error.
    ///
    /// Note: `sqlx::Error` is not cloneable, so `DatabaseError` is simplified to `RowNotFound` during cloning.
    fn clone(&self) -> Self {
        match self {
            AppError::DatabaseError(_e) => AppError::DatabaseError(sqlx::Error::RowNotFound), // Simplified clone
            AppError::NotFound(msg) => AppError::NotFound(msg.clone()),
            AppError::BadRequest(msg) => AppError::BadRequest(msg.clone()),
            AppError::ExternalApiError(msg) => AppError::ExternalApiError(msg.clone()),
            AppError::InternalError(msg) => AppError::InternalError(msg.clone()),
            AppError::Unauthorized(msg) => AppError::Unauthorized(msg.clone()),
            AppError::WithContext { source, context } => AppError::WithContext {
                source: source.clone(),
                context: context.clone(),
            },
        }
    }
}

impl From<sqlx::Error> for AppError {
    /// Converts a `sqlx::Error` into an `AppError`.
    fn from(err: sqlx::Error) -> Self {
        AppError::DatabaseError(err)
    }
}

impl From<reqwest::Error> for AppError {
    /// Converts a `reqwest::Error` into an `AppError`.
    fn from(err: reqwest::Error) -> Self {
        AppError::ExternalApiError(err.to_string())
    }
}

/// Extension trait for adding context to errors.
/// Similar to `anyhow::Context` but for our `AppError` type.
pub trait ResultExt<T> {
    /// Add context to an error.
    ///
    /// # Arguments
    ///
    /// * `context` - The context message to add.
    fn context(self, context: impl Into<String>) -> Result<T, AppError>;

    /// Add context lazily (only evaluated on error).
    ///
    /// # Arguments
    ///
    /// * `f` - A closure that produces the context message.
    #[allow(dead_code)]
    fn with_context<F>(self, f: F) -> Result<T, AppError>
    where
        F: FnOnce() -> String;
}

impl<T> ResultExt<T> for Result<T, AppError> {
    fn context(self, context: impl Into<String>) -> Result<T, AppError> {
        self.map_err(|e| AppError::WithContext {
            source: Box::new(e),
            context: context.into(),
        })
    }

    fn with_context<F>(self, f: F) -> Result<T, AppError>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| AppError::WithContext {
            source: Box::new(e),
            context: f(),
        })
    }
}

/// Extension for sqlx::Error to add context
impl<T> ResultExt<T> for Result<T, sqlx::Error> {
    fn context(self, context: impl Into<String>) -> Result<T, AppError> {
        self.map_err(|e| AppError::WithContext {
            source: Box::new(AppError::DatabaseError(e)),
            context: context.into(),
        })
    }

    fn with_context<F>(self, f: F) -> Result<T, AppError>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| AppError::WithContext {
            source: Box::new(AppError::DatabaseError(e)),
            context: f(),
        })
    }
}
