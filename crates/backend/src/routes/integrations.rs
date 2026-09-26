//! Settings-driven, per-category third-party integration configuration
//! — see `domain::integrations`'s module docs for why this exists
//! ahead of any actual provider code. Nothing here calls an SMS,
//! email, WhatsApp, payment, banking, or accounting provider; it only
//! stores how an organization *would* reach one, for real integration
//! code to read later.

use axum::extract::Path;
use axum::routing::{get, put};
use axum::{extract::State, Json, Router};
use chrono::{DateTime, Utc};
use domain::{IntegrationCategory, IntegrationConfig, UpsertIntegrationConfigInput, PERM_SETTINGS_MANAGE_INTEGRATIONS};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::error::AppError;
use crate::extractors::AuthUser;
use crate::pg_enum::{from_pg, to_pg};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/settings/integrations", get(list_integration_configs))
        .route(
            "/api/v1/settings/integrations/:category",
            put(upsert_integration_config).delete(delete_integration_config),
        )
}

#[derive(sqlx::FromRow)]
struct IntegrationConfigRow {
    id: Uuid,
    category: String,
    provider: String,
    enabled: bool,
    config: JsonValue,
    api_key: Option<String>,
    api_secret: Option<String>,
    extra_credentials: Option<JsonValue>,
    updated_at: DateTime<Utc>,
    updated_by_name: Option<String>,
}

impl IntegrationConfigRow {
    fn into_domain(self) -> Result<IntegrationConfig, AppError> {
        Ok(IntegrationConfig {
            id: self.id,
            category: from_pg("integration_configs.category", &self.category)?,
            provider: self.provider,
            enabled: self.enabled,
            config: self.config,
            has_api_key: self.api_key.is_some_and(|s| !s.is_empty()),
            has_api_secret: self.api_secret.is_some_and(|s| !s.is_empty()),
            has_extra_credentials: self.extra_credentials.is_some_and(|v| !v.is_null()),
            updated_at: self.updated_at,
            updated_by_name: self.updated_by_name,
        })
    }
}

/// Unlike `pg_enum::from_pg` (which treats an unrecognized value as an
/// internal invariant violation — appropriate for a column already
/// protected by a database check constraint), the `:category` path
/// segment is arbitrary client input and needs to fail as an ordinary
/// 400, not a 500, when it's not one of the six real categories.
fn parse_category(raw: &str) -> Result<IntegrationCategory, AppError> {
    serde_json::from_value(serde_json::Value::String(raw.to_string())).map_err(|_| {
        AppError::bad_request(format!(
            "\"{raw}\" isn't a known integration category."
        ))
    })
}

const INTEGRATION_CONFIG_QUERY: &str = r#"
    select ic.id, ic.category, ic.provider, ic.enabled, ic.config,
        ic.api_key, ic.api_secret, ic.extra_credentials, ic.updated_at,
        u.full_name as updated_by_name
    from integration_configs ic
    left join users u on u.id = ic.updated_by
"#;

async fn list_integration_configs(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<IntegrationConfig>>, AppError> {
    auth.require_permission(PERM_SETTINGS_MANAGE_INTEGRATIONS)?;

    let rows: Vec<IntegrationConfigRow> = sqlx::query_as(&format!(
        "{INTEGRATION_CONFIG_QUERY} where ic.organization_id = $1 order by ic.category"
    ))
    .bind(auth.organization_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(IntegrationConfigRow::into_domain)
            .collect::<Result<Vec<_>, AppError>>()?,
    ))
}

/// Upserts the config for one category. Credential fields are
/// tri-state by *presence*, not just `Option` — see
/// `UpsertIntegrationConfigInput`'s own doc comment: omitted keeps the
/// existing stored value (via `coalesce` against the prior row rather
/// than a plain `insert ... on conflict do update`, which would
/// otherwise overwrite every column unconditionally), present
/// (including an empty string) replaces it.
async fn upsert_integration_config(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(category): Path<String>,
    Json(input): Json<UpsertIntegrationConfigInput>,
) -> Result<Json<IntegrationConfig>, AppError> {
    auth.require_permission(PERM_SETTINGS_MANAGE_INTEGRATIONS)?;

    let category_enum = parse_category(&category)?;
    let category_pg = to_pg(&category_enum);

    let provider = input.provider.trim();
    if provider.is_empty() {
        return Err(AppError::bad_request("Enter a provider name."));
    }

    let row: IntegrationConfigRow = sqlx::query_as(&format!(
        r#"
        insert into integration_configs
            (organization_id, category, provider, enabled, config, api_key, api_secret, extra_credentials, updated_by)
        values ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        on conflict (organization_id, category) do update set
            provider = excluded.provider,
            enabled = excluded.enabled,
            config = excluded.config,
            -- `excluded.*` is SQL NULL exactly when the input field was
            -- omitted (`Option::None` binds NULL) — `coalesce` then
            -- keeps the existing stored value. A present-but-empty
            -- value (`Some("")` for a credential, `Some(Value::Null)`
            -- for extra_credentials) is NOT SQL NULL, so it overwrites
            -- as an explicit clear — see this handler's own doc comment.
            api_key = coalesce(excluded.api_key, integration_configs.api_key),
            api_secret = coalesce(excluded.api_secret, integration_configs.api_secret),
            extra_credentials = coalesce(excluded.extra_credentials, integration_configs.extra_credentials),
            updated_at = now(),
            updated_by = excluded.updated_by
        returning id, category, provider, enabled, config, api_key, api_secret, extra_credentials, updated_at,
            (select full_name from users where id = $9) as updated_by_name
        "#,
    ))
    .bind(auth.organization_id)
    .bind(&category_pg)
    .bind(provider)
    .bind(input.enabled)
    .bind(&input.config)
    .bind(&input.api_key)
    .bind(&input.api_secret)
    .bind(&input.extra_credentials)
    .bind(auth.user_id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(row.into_domain()?))
}

async fn delete_integration_config(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(category): Path<String>,
) -> Result<Json<()>, AppError> {
    auth.require_permission(PERM_SETTINGS_MANAGE_INTEGRATIONS)?;

    let category_enum = parse_category(&category)?;
    let category_pg = to_pg(&category_enum);

    sqlx::query("delete from integration_configs where organization_id = $1 and category = $2")
        .bind(auth.organization_id)
        .bind(&category_pg)
        .execute(&state.db)
        .await?;

    Ok(Json(()))
}
