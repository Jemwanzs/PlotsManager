use axum::async_trait;
use axum::extract::FromRequestParts;
use axum::http::{header, request::Parts};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::auth::verify_session_token;
use crate::error::AppError;
use crate::state::AppState;

/// The authenticated caller, extracted from a verified JWT
/// (`crates/backend/src/auth.rs`). `organization_id` rides in the token
/// itself, so every handler that uses this extractor gets it for free —
/// scope every query to it, that's the primary tenant-isolation boundary
/// (docs/10-database-and-security-design.md; RLS is defense in depth
/// behind this, not a substitute for it).
pub struct AuthUser {
    pub user_id: Uuid,
    pub organization_id: Uuid,
    pub is_platform_owner: bool,
}

#[derive(sqlx::FromRow)]
struct TenantGateRow {
    status: String,
    subscription_status: Option<String>,
    trial_ends_at: Option<DateTime<Utc>>,
}

#[async_trait]
impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or(AppError::Unauthorized)?;

        let claims =
            verify_session_token(token, &state.jwt_secret).map_err(|_| AppError::Unauthorized)?;

        // Enforced at login too, but a JWT stays valid for
        // SESSION_TTL_HOURS after that — without this, deactivating a
        // tenant or its trial expiring wouldn't take effect until every
        // already-issued token expired on its own. One extra indexed
        // lookup per authenticated request; acceptable at this scale, and
        // the correct place to revisit first if that ever changes.
        let gate: Option<TenantGateRow> = sqlx::query_as(
            r#"select o.status,
                   os.status as subscription_status,
                   os.current_period_end as trial_ends_at
               from organizations o
               left join organization_subscriptions os on os.organization_id = o.id
               where o.id = $1"#,
        )
        .bind(claims.organization_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| AppError::Unauthorized)?;

        if let Some(gate) = gate {
            crate::tenant_gate::check(
                &gate.status,
                claims.is_platform_owner,
                gate.subscription_status.as_deref(),
                gate.trial_ends_at,
            )?;
        }

        Ok(AuthUser {
            user_id: claims.sub,
            organization_id: claims.organization_id,
            is_platform_owner: claims.is_platform_owner,
        })
    }
}
