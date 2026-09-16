//! Organization / System Configuration — general settings (currency,
//! date format, timezone) plus the auto-numbering engine for plots and
//! projects. See `database/migrations/0010_organization_settings.sql`
//! and `domain::organization` for the shared shapes/formatting.
//!
//! No role check beyond "signed in, in this org" yet — there's no real
//! permission system wired to `AuthUser` until Phase 2 (roles/
//! role_assignments exist in the schema but nothing reads them today).
//! Every org member can view and edit settings for now; tightening this
//! to admins-only is exactly what Phase 2 adds, not a gap introduced
//! here.

use axum::extract::{Path, Query, State};
use axum::{routing::get, routing::post, Json, Router};
use domain::{
    format_sequence_number, GeneratedNumber, NumberingConfig, NumberingConfigInput,
    NumberingEntityType, OrganizationSettings, UpdateOrganizationSettingsInput,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::error::AppError;
use crate::extractors::AuthUser;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/settings", get(get_settings).put(update_settings))
        .route(
            "/api/v1/settings/numbering/:entity_type/next",
            post(next_number),
        )
}

#[derive(sqlx::FromRow)]
struct OrgSettingsRow {
    name: String,
    currency: String,
    date_format: String,
    timezone: String,
}

#[derive(sqlx::FromRow, Clone)]
struct NumberingRow {
    entity_type: String,
    prefix: String,
    include_year: bool,
    include_entity_code: bool,
    padding: i32,
    next_number: i32,
}

impl NumberingRow {
    /// Preview-only: shows what the *next* number would look like
    /// without consuming it. When plot numbering interpolates a project
    /// code and there's no specific project in view (this is the
    /// org-wide settings screen, not a create-plot form), "ABC" stands
    /// in as a visibly-a-placeholder example.
    fn into_config(self) -> Result<NumberingConfig, AppError> {
        let entity_type = match self.entity_type.as_str() {
            "plot" => NumberingEntityType::Plot,
            "project" => NumberingEntityType::Project,
            other => {
                return Err(AppError::Internal(anyhow::anyhow!(
                    "unknown numbering entity_type {other}"
                )))
            }
        };
        let placeholder_code = (entity_type == NumberingEntityType::Plot
            && self.include_entity_code)
            .then_some("ABC");
        let preview = format_sequence_number(
            &self.prefix,
            self.include_year,
            placeholder_code,
            self.padding as u32,
            self.next_number as u32,
        );
        Ok(NumberingConfig {
            entity_type,
            prefix: self.prefix,
            include_year: self.include_year,
            include_entity_code: self.include_entity_code,
            padding: self.padding as u32,
            next_number: self.next_number as u32,
            preview,
        })
    }
}

async fn fetch_settings(
    state: &AppState,
    organization_id: Uuid,
) -> Result<OrganizationSettings, AppError> {
    let org: OrgSettingsRow = sqlx::query_as(
        "select name, currency, date_format, timezone from organizations where id = $1",
    )
    .bind(organization_id)
    .fetch_one(&state.db)
    .await?;

    let numbering: Vec<NumberingRow> = sqlx::query_as(
        r#"select entity_type, prefix, include_year, include_entity_code, padding, next_number
           from numbering_sequences where organization_id = $1"#,
    )
    .bind(organization_id)
    .fetch_all(&state.db)
    .await?;

    let plot_row = numbering
        .iter()
        .find(|r| r.entity_type == "plot")
        .cloned()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("missing plot numbering config")))?;
    let project_row = numbering
        .iter()
        .find(|r| r.entity_type == "project")
        .cloned()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("missing project numbering config")))?;

    Ok(OrganizationSettings {
        organization_id,
        name: org.name,
        currency: org.currency,
        date_format: org.date_format,
        timezone: org.timezone,
        plot_numbering: plot_row.into_config()?,
        project_numbering: project_row.into_config()?,
    })
}

async fn get_settings(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<OrganizationSettings>, AppError> {
    Ok(Json(fetch_settings(&state, auth.organization_id).await?))
}

fn validate_numbering(input: &NumberingConfigInput) -> Result<(), AppError> {
    if input.prefix.trim().len() > 20 {
        return Err(AppError::bad_request(
            "Numbering prefix must be 20 characters or fewer.",
        ));
    }
    if !(1..=10).contains(&input.padding) {
        return Err(AppError::bad_request(
            "Numbering digit padding must be between 1 and 10.",
        ));
    }
    if input.next_number == 0 {
        return Err(AppError::bad_request(
            "The next number to issue must be at least 1.",
        ));
    }
    Ok(())
}

async fn update_settings(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<UpdateOrganizationSettingsInput>,
) -> Result<Json<OrganizationSettings>, AppError> {
    let currency = input.currency.trim().to_uppercase();
    if currency.len() < 2 || currency.len() > 5 || !currency.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err(AppError::bad_request(
            "Currency must be a 2-5 letter code, e.g. KES or USD.",
        ));
    }
    let date_format = input.date_format.trim();
    let timezone = input.timezone.trim();
    if date_format.is_empty() || timezone.is_empty() {
        return Err(AppError::bad_request(
            "Enter a date format and time zone.",
        ));
    }
    validate_numbering(&input.plot_numbering)?;
    validate_numbering(&input.project_numbering)?;

    let mut tx = state.db.begin().await?;

    sqlx::query(
        "update organizations set currency = $1, date_format = $2, timezone = $3 where id = $4",
    )
    .bind(&currency)
    .bind(date_format)
    .bind(timezone)
    .bind(auth.organization_id)
    .execute(&mut *tx)
    .await?;

    for (entity_type, cfg) in [
        ("plot", &input.plot_numbering),
        ("project", &input.project_numbering),
    ] {
        sqlx::query(
            r#"update numbering_sequences
               set prefix = $1, include_year = $2, include_entity_code = $3,
                   padding = $4, next_number = $5
               where organization_id = $6 and entity_type = $7"#,
        )
        .bind(cfg.prefix.trim())
        .bind(cfg.include_year)
        .bind(cfg.include_entity_code)
        .bind(cfg.padding as i32)
        .bind(cfg.next_number as i32)
        .bind(auth.organization_id)
        .bind(entity_type)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(Json(fetch_settings(&state, auth.organization_id).await?))
}

#[derive(Deserialize)]
struct NextNumberQuery {
    /// Only meaningful when `:entity_type` is `plot` and that org's plot
    /// numbering has `include_entity_code` on — the owning project's
    /// `code`, supplied by the create-plot form since the backend has no
    /// other way to know which project this generated number is for
    /// (the counter itself is org-wide, not per-project).
    project_code: Option<String>,
}

/// Atomically consumes the next number for `entity_type` and returns it
/// formatted — used by the "Auto-generate" affordance on the create-plot
/// and create-project forms to pre-fill the number field (still editable,
/// still subject to the normal uniqueness check on submit). If the form
/// is never submitted, that number is simply skipped — gaps are fine,
/// reuse is not (see migration 0010's module docs).
async fn next_number(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(entity_type): Path<String>,
    Query(query): Query<NextNumberQuery>,
) -> Result<Json<GeneratedNumber>, AppError> {
    if !matches!(entity_type.as_str(), "plot" | "project") {
        return Err(AppError::bad_request(
            "Unsupported numbering entity type.",
        ));
    }

    let row: NumberingRow = sqlx::query_as(
        r#"update numbering_sequences
           set next_number = next_number + 1
           where organization_id = $1 and entity_type = $2
           returning entity_type, prefix, include_year, include_entity_code,
               padding, next_number - 1 as next_number"#,
    )
    .bind(auth.organization_id)
    .bind(&entity_type)
    .fetch_one(&state.db)
    .await?;

    let entity_code = if entity_type == "plot" {
        query.project_code.as_deref()
    } else {
        None
    };

    let number = format_sequence_number(
        &row.prefix,
        row.include_year,
        entity_code,
        row.padding as u32,
        row.next_number as u32,
    );

    Ok(Json(GeneratedNumber { number }))
}
