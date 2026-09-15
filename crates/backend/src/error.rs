use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

/// Backend-internal error type — maps to an HTTP status + `{"error": "..."}"`
/// body. Deliberately **not** `domain::ApiError`: that type is the shape
/// the frontend deserializes a *known* failure into (invalid credentials,
/// not found, ...), constructed client-side from this response's status
/// code and message (see `crates/frontend/src/api/http.rs`) — the two
/// crates can't share one type here anyway (implementing `IntoResponse`
/// for a foreign type from a third crate violates the orphan rule), so
/// don't try to force it; keep the HTTP-shape concern here and the
/// wire-contract concern in `domain`.
#[derive(Debug)]
pub enum AppError {
    BadRequest(String),
    Unauthorized,
    /// Authenticated, but not allowed to do this — distinct from
    /// `Unauthorized` (not signed in at all). Currently only used to gate
    /// `/api/v1/platform/*` behind `AuthUser.is_platform_owner`.
    Forbidden(String),
    NotFound,
    Conflict(String),
    Internal(anyhow::Error),
}

impl AppError {
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::BadRequest(msg.into())
    }

    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self::Forbidden(msg.into())
    }

    pub fn conflict(msg: impl Into<String>) -> Self {
        Self::Conflict(msg.into())
    }

    fn status(&self) -> StatusCode {
        match self {
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::Unauthorized => StatusCode::UNAUTHORIZED,
            AppError::Forbidden(_) => StatusCode::FORBIDDEN,
            AppError::NotFound => StatusCode::NOT_FOUND,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// The user-facing message alone, without wrapping it in an HTTP
    /// response — a bulk-import endpoint (`routes/projects.rs`'s
    /// `bulk_create_plots`, `routes/customers.rs`'s
    /// `bulk_create_customers`) attempts one row at a time and reports
    /// each failure's message in a 200 OK summary rather than as its
    /// own HTTP error, so it needs this without `into_response`'s
    /// status-code wrapping.
    pub fn client_message(&self) -> String {
        match self {
            AppError::BadRequest(msg) => msg.clone(),
            AppError::Unauthorized => "not signed in".to_string(),
            AppError::Forbidden(msg) => msg.clone(),
            AppError::NotFound => "not found".to_string(),
            AppError::Conflict(msg) => msg.clone(),
            AppError::Internal(err) => {
                tracing::error!("internal error: {err:#}");
                "something went wrong".to_string()
            }
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status();
        let message = self.client_message();
        (status, Json(json!({ "error": message }))).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        match &err {
            sqlx::Error::RowNotFound => AppError::NotFound,
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                AppError::Conflict("that record already exists".to_string())
            }
            _ => AppError::Internal(err.into()),
        }
    }
}
