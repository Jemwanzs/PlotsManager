use std::collections::HashSet;

use axum::async_trait;
use axum::extract::FromRequestParts;
use axum::http::{header, request::Parts};
use chrono::{DateTime, Utc};
use domain::PERM_WILDCARD;
use uuid::Uuid;

use crate::auth::verify_session_token;
use crate::error::AppError;
use crate::state::AppState;

/// The union of every `roles.permissions` array across a user's
/// `role_assignments` — shared by this extractor (runs per request)
/// and by `routes/auth.rs::login` / `routes/account.rs::change_password`
/// (runs once, to populate `User.permissions` so the frontend can hide
/// actions a role doesn't grant instead of only finding out from a
/// 403 after clicking).
pub async fn fetch_permissions(db: &sqlx::PgPool, user_id: Uuid) -> Result<HashSet<String>, sqlx::Error> {
    let permission_rows: Vec<(serde_json::Value,)> = sqlx::query_as(
        r#"select r.permissions
           from role_assignments ra
           join roles r on r.id = ra.role_id
           where ra.user_id = $1"#,
    )
    .bind(user_id)
    .fetch_all(db)
    .await?;

    Ok(permission_rows
        .into_iter()
        .flat_map(|(perms,)| {
            perms
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|v| v.as_str().map(str::to_string))
        })
        .collect())
}

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
    /// Union of every `roles.permissions` array across this user's
    /// `role_assignments` (`database/migrations/0001_init.sql`) —
    /// looked up fresh per request (one extra indexed query, same
    /// tradeoff as the tenant-gate lookup below) rather than cached in
    /// the JWT, so revoking a role takes effect on the very next
    /// request instead of waiting out the token's TTL. `"*"` (the
    /// signup-provisioned "Admin" role's only permission) grants
    /// everything — see `has_permission`.
    pub permissions: HashSet<String>,
}

impl AuthUser {
    pub fn has_permission(&self, permission: &str) -> bool {
        self.is_platform_owner
            || self.permissions.contains(PERM_WILDCARD)
            || self.permissions.contains(permission)
    }

    pub fn require_permission(&self, permission: &str) -> Result<(), AppError> {
        if self.has_permission(permission) {
            Ok(())
        } else {
            Err(AppError::forbidden(
                "You don't have permission to do this.",
            ))
        }
    }
}

#[derive(sqlx::FromRow)]
struct TenantGateRow {
    status: String,
    subscription_status: Option<String>,
    trial_ends_at: Option<DateTime<Utc>>,
    session_valid_after: DateTime<Utc>,
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
                   os.current_period_end as trial_ends_at,
                   u.session_valid_after
               from organizations o
               join users u on u.organization_id = o.id and u.id = $2
               left join organization_subscriptions os on os.organization_id = o.id
               where o.id = $1"#,
        )
        .bind(claims.organization_id)
        .bind(claims.sub)
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

            // A password reset or an explicit "Revoke sessions" bumps
            // this to now() — any token issued before that (`iat`) is
            // stale, even though it hasn't hit its own `exp` yet.
            if claims.iat < gate.session_valid_after.timestamp() {
                return Err(AppError::Unauthorized);
            }
        }

        let permissions = fetch_permissions(&state.db, claims.sub)
            .await
            .map_err(|_| AppError::Unauthorized)?;

        Ok(AuthUser {
            user_id: claims.sub,
            organization_id: claims.organization_id,
            is_platform_owner: claims.is_platform_owner,
            permissions,
        })
    }
}
