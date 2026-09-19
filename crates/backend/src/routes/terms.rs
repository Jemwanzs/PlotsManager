//! Public (unauthenticated) — the sign-up page needs to show the
//! current Terms & Conditions and record which version the applicant
//! accepted before an organization/user exists yet to gate this behind
//! a session.

use axum::{extract::State, routing::get, Json, Router};
use domain::TermsVersion;
use uuid::Uuid;

use crate::error::AppError;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/v1/terms/current", get(current_terms))
}

#[derive(sqlx::FromRow)]
struct TermsVersionRow {
    id: Uuid,
    version_label: String,
    body: String,
}

async fn current_terms(State(state): State<AppState>) -> Result<Json<TermsVersion>, AppError> {
    let row: TermsVersionRow = sqlx::query_as(
        "select id, version_label, body from terms_versions where is_current = true",
    )
    .fetch_one(&state.db)
    .await?;
    Ok(Json(TermsVersion {
        id: row.id,
        version_label: row.version_label,
        body: row.body,
    }))
}
