//! A plot's title/ownership history — see `domain::title_record`'s
//! module docs and `database/migrations/0028_title_records.sql` for
//! the shape rationale (structured tracking alongside, not replacing,
//! `plots.title_number`).
//!
//! No delete route: title records are a provenance trail (who was
//! registered owner when, per the legacy-migration-readiness "preserve
//! historical provenance" instruction) — a wrong entry gets corrected
//! via `update_title_record`, never removed.

use axum::extract::{Path, State};
use axum::routing::{get, put};
use axum::{Json, Router};
use chrono::{DateTime, NaiveDate, Utc};
use domain::{
    CreateTitleRecordInput, TitleRecord, UpdateTitleRecordInput, PERM_PLOTS_VIEW, PERM_TITLES_MANAGE,
};
use uuid::Uuid;

use crate::error::AppError;
use crate::extractors::AuthUser;
use crate::pg_enum::{from_pg, to_pg};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/plots/:plot_id/title-records",
            get(list_title_records).post(create_title_record),
        )
        .route("/api/v1/title-records/:id", put(update_title_record))
}

async fn plot_organization_id(db: &sqlx::PgPool, plot_id: Uuid) -> Result<Option<Uuid>, AppError> {
    Ok(sqlx::query_scalar(
        "select p.organization_id from plots pl join projects p on p.id = pl.project_id where pl.id = $1",
    )
    .bind(plot_id)
    .fetch_optional(db)
    .await?)
}

#[derive(sqlx::FromRow)]
struct TitleRecordRow {
    id: Uuid,
    plot_id: Uuid,
    title_number: String,
    registered_owner_name: String,
    previous_owner_name: Option<String>,
    title_status: String,
    transfer_status: String,
    issue_date: Option<NaiveDate>,
    registration_date: Option<NaiveDate>,
    transfer_date: Option<NaiveDate>,
    notes: Option<String>,
    created_by_name: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl TitleRecordRow {
    fn into_domain(self) -> Result<TitleRecord, AppError> {
        Ok(TitleRecord {
            id: self.id,
            plot_id: self.plot_id,
            title_number: self.title_number,
            registered_owner_name: self.registered_owner_name,
            previous_owner_name: self.previous_owner_name,
            title_status: from_pg("title_status", &self.title_status)?,
            transfer_status: from_pg("transfer_status", &self.transfer_status)?,
            issue_date: self.issue_date,
            registration_date: self.registration_date,
            transfer_date: self.transfer_date,
            notes: self.notes,
            created_by_name: self.created_by_name,
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
}

const TITLE_RECORD_COLUMNS: &str = "t.id, t.plot_id, t.title_number, t.registered_owner_name, t.previous_owner_name,
    t.title_status, t.transfer_status, t.issue_date, t.registration_date, t.transfer_date, t.notes,
    u.full_name as created_by_name, t.created_at, t.updated_at";

async fn list_title_records(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(plot_id): Path<Uuid>,
) -> Result<Json<Vec<TitleRecord>>, AppError> {
    auth.require_permission(PERM_PLOTS_VIEW)?;
    let Some(org_id) = plot_organization_id(&state.db, plot_id).await? else {
        return Err(AppError::NotFound);
    };
    if org_id != auth.organization_id {
        return Err(AppError::NotFound);
    }

    let rows: Vec<TitleRecordRow> = sqlx::query_as(&format!(
        "select {TITLE_RECORD_COLUMNS} from title_records t
         join users u on u.id = t.created_by
         where t.plot_id = $1
         order by t.created_at desc"
    ))
    .bind(plot_id)
    .fetch_all(&state.db)
    .await?;

    rows.into_iter().map(TitleRecordRow::into_domain).collect::<Result<Vec<_>, _>>().map(Json)
}

async fn create_title_record(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(plot_id): Path<Uuid>,
    Json(input): Json<CreateTitleRecordInput>,
) -> Result<Json<TitleRecord>, AppError> {
    auth.require_permission(PERM_TITLES_MANAGE)?;
    let Some(org_id) = plot_organization_id(&state.db, plot_id).await? else {
        return Err(AppError::NotFound);
    };
    if org_id != auth.organization_id {
        return Err(AppError::NotFound);
    }
    if input.title_number.trim().is_empty() {
        return Err(AppError::bad_request("Title number is required."));
    }
    if input.registered_owner_name.trim().is_empty() {
        return Err(AppError::bad_request("Registered owner is required."));
    }

    let row: TitleRecordRow = sqlx::query_as(&format!(
        r#"
        with inserted as (
            insert into title_records (
                organization_id, plot_id, title_number, registered_owner_name, previous_owner_name,
                title_status, transfer_status, issue_date, registration_date, transfer_date, notes, created_by
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            returning *
        )
        select {TITLE_RECORD_COLUMNS} from inserted t join users u on u.id = t.created_by
        "#,
    ))
    .bind(auth.organization_id)
    .bind(plot_id)
    .bind(input.title_number.trim())
    .bind(input.registered_owner_name.trim())
    .bind(input.previous_owner_name.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(to_pg(&input.title_status))
    .bind(to_pg(&input.transfer_status))
    .bind(input.issue_date)
    .bind(input.registration_date)
    .bind(input.transfer_date)
    .bind(input.notes.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(auth.user_id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(row.into_domain()?))
}

async fn update_title_record(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateTitleRecordInput>,
) -> Result<Json<TitleRecord>, AppError> {
    auth.require_permission(PERM_TITLES_MANAGE)?;
    if input.title_number.trim().is_empty() {
        return Err(AppError::bad_request("Title number is required."));
    }
    if input.registered_owner_name.trim().is_empty() {
        return Err(AppError::bad_request("Registered owner is required."));
    }

    let row: Option<TitleRecordRow> = sqlx::query_as(&format!(
        r#"
        with updated as (
            update title_records set
                title_number = $1,
                registered_owner_name = $2,
                previous_owner_name = $3,
                title_status = $4,
                transfer_status = $5,
                issue_date = $6,
                registration_date = $7,
                transfer_date = $8,
                notes = $9,
                updated_at = now()
            where id = $10 and organization_id = $11
            returning *
        )
        select {TITLE_RECORD_COLUMNS} from updated t join users u on u.id = t.created_by
        "#,
    ))
    .bind(input.title_number.trim())
    .bind(input.registered_owner_name.trim())
    .bind(input.previous_owner_name.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(to_pg(&input.title_status))
    .bind(to_pg(&input.transfer_status))
    .bind(input.issue_date)
    .bind(input.registration_date)
    .bind(input.transfer_date)
    .bind(input.notes.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(id)
    .bind(auth.organization_id)
    .fetch_optional(&state.db)
    .await?;

    match row {
        Some(row) => Ok(Json(row.into_domain()?)),
        None => Err(AppError::NotFound),
    }
}
