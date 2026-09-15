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
    NotFound,
    Conflict(String),
    Internal(anyhow::Error),
}

impl AppError {
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::BadRequest(msg.into())
    }

    pub fn conflict(msg: impl Into<String>) -> Self {
        Self::Conflict(msg.into())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "not signed in".to_string()),
            AppError::NotFound => (StatusCode::NOT_FOUND, "not found".to_string()),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, msg.clone()),
            AppError::Internal(err) => {
                tracing::error!("internal error: {err:#}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "something went wrong".to_string(),
                )
            }
        };
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
