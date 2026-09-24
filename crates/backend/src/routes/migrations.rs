//! The legacy-data staging/migration framework — see
//! `domain::migration`'s module docs and `database/migrations/
//! 0029_migration_framework.sql` for the shape rationale.
//!
//! Flow: `create_batch` (upload) parses+validates+normalizes every row
//! immediately (deterministic, header-based — see `normalize_customer_row`)
//! and stages it; nothing touches `customers` yet. `update_staging_row`
//! is "resolve exceptions" — edit a row's fields and re-validate in
//! place. `commit_batch` is the only thing that writes to `customers`,
//! and only for rows currently `valid`; it re-validates once more right
//! before inserting (a legacy_customer_number that collided with a
//! customer added after the batch was staged becomes a fresh exception
//! rather than a hard failure of the whole commit).
//!
//! Customer-only today (`entity_type` check constraint enforces this at
//! the schema level too) — Plot/Project/Sale/LoanAccount would each get
//! their own `normalize_*_row`/commit-insert pair following this same
//! shape, dispatched on `MigrationEntityType`.

use std::collections::{HashMap, HashSet};

use axum::extract::{Path, State};
use axum::routing::{get, put};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use domain::{
    CreateMigrationBatchInput, MigrationBatch, MigrationCommitResult, MigrationEntityType,
    MigrationStagingRow, UpdateMigrationRowInput, PERM_MIGRATIONS_MANAGE,
};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::error::AppError;
use crate::extractors::AuthUser;
use crate::pg_enum::{from_pg, to_pg};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/migrations", get(list_batches).post(create_batch))
        .route("/api/v1/migrations/:id", get(get_batch))
        .route("/api/v1/migrations/:id/rows", get(list_rows))
        .route("/api/v1/migrations/:id/rows/:row_id", put(update_staging_row))
        .route("/api/v1/migrations/:id/commit", axum::routing::post(commit_batch))
}

fn get_field<'a>(raw: &'a HashMap<String, String>, key: &str) -> Option<&'a str> {
    raw.get(key).map(|s| s.trim()).filter(|s| !s.is_empty())
}

/// Validates and normalizes one uploaded customer row. Returns the
/// normalized JSON (kept even on failure — a human reviewing the
/// exception should see what was actually parsed out, not just the
/// raw text) plus a status and, on failure, every problem found (not
/// just the first) so "resolve exceptions" doesn't become a game of
/// fix-one-resubmit-find-the-next.
///
/// `existing_legacy_numbers` is every legacy_customer_number already on
/// a customer in this org; `seen_in_batch` accumulates as rows are
/// walked, so a duplicate *within the uploaded file itself* is caught
/// too, not just a collision with pre-existing data.
fn normalize_customer_row(
    raw: &HashMap<String, String>,
    existing_legacy_numbers: &HashSet<String>,
    seen_in_batch: &mut HashSet<String>,
) -> (JsonValue, &'static str, Option<String>) {
    let mut errors = Vec::new();

    let full_name = get_field(raw, "full_name");
    if full_name.is_none() {
        errors.push("Missing full_name".to_string());
    }

    let legacy_number = get_field(raw, "legacy_customer_number");
    match legacy_number {
        None => errors.push("Missing legacy_customer_number".to_string()),
        Some(num) => {
            if existing_legacy_numbers.contains(num) {
                errors.push(format!(
                    "legacy_customer_number \"{num}\" already exists on a customer in this organization"
                ));
            } else if !seen_in_batch.insert(num.to_string()) {
                errors.push(format!("legacy_customer_number \"{num}\" is duplicated within this file"));
            }
        }
    }

    let customer_type_raw = get_field(raw, "customer_type").map(|s| s.to_lowercase());
    let customer_type = match customer_type_raw.as_deref() {
        None => "individual",
        Some("individual") => "individual",
        Some("company") => "company",
        Some("joint") => "joint",
        Some(other) => {
            errors.push(format!(
                "customer_type \"{other}\" isn't recognised — use individual, company, or joint"
            ));
            "individual"
        }
    };

    let normalized = serde_json::json!({
        "full_name": full_name,
        "legacy_customer_number": legacy_number,
        "id_number": get_field(raw, "id_number"),
        "email": get_field(raw, "email"),
        "phone": get_field(raw, "phone"),
        "title": get_field(raw, "title"),
        "customer_type": customer_type,
        "kra_pin": get_field(raw, "kra_pin"),
        "postal_address": get_field(raw, "postal_address"),
        "city": get_field(raw, "city"),
        "physical_address": get_field(raw, "physical_address"),
        "next_of_kin_name": get_field(raw, "next_of_kin_name"),
        "next_of_kin_relationship": get_field(raw, "next_of_kin_relationship"),
        "next_of_kin_mobile": get_field(raw, "next_of_kin_mobile"),
        "next_of_kin_id_number": get_field(raw, "next_of_kin_id_number"),
        "next_of_kin_address": get_field(raw, "next_of_kin_address"),
    });

    if errors.is_empty() {
        (normalized, "valid", None)
    } else {
        (normalized, "exception", Some(errors.join("; ")))
    }
}

async fn existing_customer_legacy_numbers(
    db: &sqlx::PgPool,
    organization_id: Uuid,
) -> Result<HashSet<String>, AppError> {
    let rows: Vec<String> = sqlx::query_scalar(
        "select legacy_customer_number from customers where organization_id = $1 and legacy_customer_number is not null",
    )
    .bind(organization_id)
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().collect())
}

#[derive(sqlx::FromRow)]
struct BatchRow {
    id: Uuid,
    entity_type: String,
    source_system: String,
    source_file_name: String,
    status: String,
    total_rows: i32,
    valid_rows: i32,
    exception_rows: i32,
    committed_rows: i32,
    created_by_name: String,
    created_at: DateTime<Utc>,
    committed_at: Option<DateTime<Utc>>,
}

impl BatchRow {
    fn into_domain(self) -> Result<MigrationBatch, AppError> {
        Ok(MigrationBatch {
            id: self.id,
            entity_type: from_pg("entity_type", &self.entity_type)?,
            source_system: self.source_system,
            source_file_name: self.source_file_name,
            status: from_pg("status", &self.status)?,
            total_rows: self.total_rows,
            valid_rows: self.valid_rows,
            exception_rows: self.exception_rows,
            committed_rows: self.committed_rows,
            created_by_name: self.created_by_name,
            created_at: self.created_at,
            committed_at: self.committed_at,
        })
    }
}

const BATCH_COLUMNS: &str = "b.id, b.entity_type, b.source_system, b.source_file_name, b.status,
    b.total_rows, b.valid_rows, b.exception_rows, b.committed_rows,
    u.full_name as created_by_name, b.created_at, b.committed_at";

async fn batch_organization_id(db: &sqlx::PgPool, batch_id: Uuid) -> Result<Option<Uuid>, AppError> {
    Ok(sqlx::query_scalar("select organization_id from migration_batches where id = $1")
        .bind(batch_id)
        .fetch_optional(db)
        .await?)
}

async fn list_batches(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<MigrationBatch>>, AppError> {
    auth.require_permission(PERM_MIGRATIONS_MANAGE)?;

    let rows: Vec<BatchRow> = sqlx::query_as(&format!(
        "select {BATCH_COLUMNS} from migration_batches b
         join users u on u.id = b.created_by
         where b.organization_id = $1
         order by b.created_at desc"
    ))
    .bind(auth.organization_id)
    .fetch_all(&state.db)
    .await?;

    rows.into_iter().map(BatchRow::into_domain).collect::<Result<Vec<_>, _>>().map(Json)
}

async fn get_batch(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<MigrationBatch>, AppError> {
    auth.require_permission(PERM_MIGRATIONS_MANAGE)?;
    let Some(org_id) = batch_organization_id(&state.db, id).await? else {
        return Err(AppError::NotFound);
    };
    if org_id != auth.organization_id {
        return Err(AppError::NotFound);
    }

    let row: BatchRow = sqlx::query_as(&format!(
        "select {BATCH_COLUMNS} from migration_batches b join users u on u.id = b.created_by where b.id = $1"
    ))
    .bind(id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(row.into_domain()?))
}

async fn create_batch(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<CreateMigrationBatchInput>,
) -> Result<Json<MigrationBatch>, AppError> {
    auth.require_permission(PERM_MIGRATIONS_MANAGE)?;

    if !matches!(input.entity_type, MigrationEntityType::Customer) {
        return Err(AppError::bad_request("Only customer migration is supported today."));
    }
    if input.rows.is_empty() {
        return Err(AppError::bad_request("The uploaded file has no data rows."));
    }
    if input.source_file_name.trim().is_empty() || input.source_system.trim().is_empty() {
        return Err(AppError::bad_request("Missing source file name or source system."));
    }

    let existing_legacy_numbers = existing_customer_legacy_numbers(&state.db, auth.organization_id).await?;
    let mut seen_in_batch = HashSet::new();

    let mut tx = state.db.begin().await?;

    let batch_id: Uuid = sqlx::query_scalar(
        "insert into migration_batches (organization_id, entity_type, source_system, source_file_name, created_by)
         values ($1, $2, $3, $4, $5) returning id",
    )
    .bind(auth.organization_id)
    .bind(to_pg(&input.entity_type))
    .bind(input.source_system.trim())
    .bind(input.source_file_name.trim())
    .bind(auth.user_id)
    .fetch_one(&mut *tx)
    .await?;

    let mut valid_count = 0i32;
    let mut exception_count = 0i32;

    for row in &input.rows {
        let raw_json = serde_json::to_value(&row.raw_data).unwrap_or(JsonValue::Null);
        let (normalized, status, message) =
            normalize_customer_row(&row.raw_data, &existing_legacy_numbers, &mut seen_in_batch);
        if status == "valid" {
            valid_count += 1;
        } else {
            exception_count += 1;
        }

        sqlx::query(
            "insert into migration_staging_rows (batch_id, source_row, raw_data, normalized_data, status, exception_message)
             values ($1, $2, $3, $4, $5, $6)",
        )
        .bind(batch_id)
        .bind(row.source_row as i32)
        .bind(raw_json)
        .bind(normalized)
        .bind(status)
        .bind(message)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query("update migration_batches set total_rows = $1, valid_rows = $2, exception_rows = $3 where id = $4")
        .bind(input.rows.len() as i32)
        .bind(valid_count)
        .bind(exception_count)
        .bind(batch_id)
        .execute(&mut *tx)
        .await?;

    let row: BatchRow = sqlx::query_as(&format!(
        "select {BATCH_COLUMNS} from migration_batches b join users u on u.id = b.created_by where b.id = $1"
    ))
    .bind(batch_id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(row.into_domain()?))
}

#[derive(sqlx::FromRow)]
struct StagingRowRow {
    id: Uuid,
    batch_id: Uuid,
    source_row: i32,
    raw_data: JsonValue,
    normalized_data: JsonValue,
    status: String,
    exception_message: Option<String>,
    committed_entity_id: Option<Uuid>,
}

impl StagingRowRow {
    fn into_domain(self) -> Result<MigrationStagingRow, AppError> {
        Ok(MigrationStagingRow {
            id: self.id,
            batch_id: self.batch_id,
            source_row: self.source_row,
            raw_data: self.raw_data,
            normalized_data: self.normalized_data,
            status: from_pg("status", &self.status)?,
            exception_message: self.exception_message,
            committed_entity_id: self.committed_entity_id,
        })
    }
}

const STAGING_ROW_COLUMNS: &str = "id, batch_id, source_row, raw_data, normalized_data, status, exception_message, committed_entity_id";

async fn list_rows(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<MigrationStagingRow>>, AppError> {
    auth.require_permission(PERM_MIGRATIONS_MANAGE)?;
    let Some(org_id) = batch_organization_id(&state.db, id).await? else {
        return Err(AppError::NotFound);
    };
    if org_id != auth.organization_id {
        return Err(AppError::NotFound);
    }

    let rows: Vec<StagingRowRow> = sqlx::query_as(&format!(
        "select {STAGING_ROW_COLUMNS} from migration_staging_rows where batch_id = $1 order by source_row asc"
    ))
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    rows.into_iter().map(StagingRowRow::into_domain).collect::<Result<Vec<_>, _>>().map(Json)
}

/// "Resolve exceptions": overwrite a row's fields (same shape as an
/// uploaded row) and re-validate in place. Works on a `valid` row too
/// (e.g. tidying a name before commit), not just exceptions.
async fn update_staging_row(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((id, row_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<UpdateMigrationRowInput>,
) -> Result<Json<MigrationStagingRow>, AppError> {
    auth.require_permission(PERM_MIGRATIONS_MANAGE)?;
    let Some(org_id) = batch_organization_id(&state.db, id).await? else {
        return Err(AppError::NotFound);
    };
    if org_id != auth.organization_id {
        return Err(AppError::NotFound);
    }

    let existing_legacy_numbers = existing_customer_legacy_numbers(&state.db, auth.organization_id).await?;
    // A row editing its own legacy_customer_number back to what it
    // already staged as shouldn't trip the "duplicated within this
    // file" check against itself — collect every OTHER row's number
    // in this batch first.
    let other_numbers: Vec<String> = sqlx::query_scalar(
        "select normalized_data->>'legacy_customer_number' from migration_staging_rows
         where batch_id = $1 and id <> $2 and normalized_data->>'legacy_customer_number' is not null",
    )
    .bind(id)
    .bind(row_id)
    .fetch_all(&state.db)
    .await?;
    let mut seen_in_batch: HashSet<String> = other_numbers.into_iter().collect();

    let raw_json = serde_json::to_value(&input.fields).unwrap_or(JsonValue::Null);
    let (normalized, status, message) =
        normalize_customer_row(&input.fields, &existing_legacy_numbers, &mut seen_in_batch);

    let old_status: Option<String> =
        sqlx::query_scalar("select status from migration_staging_rows where id = $1 and batch_id = $2")
            .bind(row_id)
            .bind(id)
            .fetch_optional(&state.db)
            .await?;
    let Some(old_status) = old_status else {
        return Err(AppError::NotFound);
    };

    let mut tx = state.db.begin().await?;

    let row: StagingRowRow = sqlx::query_as(&format!(
        "update migration_staging_rows set raw_data = $1, normalized_data = $2, status = $3, exception_message = $4
         where id = $5 and batch_id = $6
         returning {STAGING_ROW_COLUMNS}"
    ))
    .bind(raw_json)
    .bind(normalized)
    .bind(status)
    .bind(message)
    .bind(row_id)
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;

    if old_status != status {
        let (valid_delta, exception_delta) = if status == "valid" { (1, -1) } else { (-1, 1) };
        sqlx::query("update migration_batches set valid_rows = valid_rows + $1, exception_rows = exception_rows + $2 where id = $3")
            .bind(valid_delta)
            .bind(exception_delta)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;

    Ok(Json(row.into_domain()?))
}

async fn commit_batch(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<MigrationCommitResult>, AppError> {
    auth.require_permission(PERM_MIGRATIONS_MANAGE)?;
    let Some(org_id) = batch_organization_id(&state.db, id).await? else {
        return Err(AppError::NotFound);
    };
    if org_id != auth.organization_id {
        return Err(AppError::NotFound);
    }

    let rows: Vec<StagingRowRow> = sqlx::query_as(&format!(
        "select {STAGING_ROW_COLUMNS} from migration_staging_rows where batch_id = $1 order by source_row asc"
    ))
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    let mut committed = 0u32;
    let mut skipped_exceptions = 0u32;
    let mut already_committed = 0u32;
    let mut seen_in_batch: HashSet<String> = HashSet::new();

    let mut tx = state.db.begin().await?;

    for row in rows {
        if row.committed_entity_id.is_some() {
            already_committed += 1;
            continue;
        }
        if row.status != "valid" {
            skipped_exceptions += 1;
            continue;
        }

        let n = &row.normalized_data;
        let legacy_number = n.get("legacy_customer_number").and_then(|v| v.as_str());

        // Re-check right before insert: data staged earlier can have
        // been overtaken by a customer created (or another row in this
        // same commit) in the meantime. A collision here becomes a
        // fresh exception, never a silent skip or a broken commit.
        if let Some(num) = legacy_number {
            let taken: bool = sqlx::query_scalar(
                "select exists(select 1 from customers where organization_id = $1 and legacy_customer_number = $2)",
            )
            .bind(auth.organization_id)
            .bind(num)
            .fetch_one(&mut *tx)
            .await?;
            if taken || !seen_in_batch.insert(num.to_string()) {
                let msg = format!("legacy_customer_number \"{num}\" was taken by another record before this commit ran");
                sqlx::query("update migration_staging_rows set status = 'exception', exception_message = $1 where id = $2")
                    .bind(&msg)
                    .bind(row.id)
                    .execute(&mut *tx)
                    .await?;
                sqlx::query("update migration_batches set valid_rows = valid_rows - 1, exception_rows = exception_rows + 1 where id = $1")
                    .bind(id)
                    .execute(&mut *tx)
                    .await?;
                skipped_exceptions += 1;
                continue;
            }
        }

        let full_name = n.get("full_name").and_then(|v| v.as_str()).unwrap_or_default();
        let get_str = |key: &str| n.get(key).and_then(|v| v.as_str());

        let customer_id: Uuid = sqlx::query_scalar(
            r#"
            insert into customers (
                organization_id, full_name, email, phone, id_number, migration_batch_id,
                title, customer_type, kra_pin, postal_address, city, physical_address,
                legacy_customer_number, next_of_kin_name, next_of_kin_relationship,
                next_of_kin_mobile, next_of_kin_id_number, next_of_kin_address
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
            returning id
            "#,
        )
        .bind(auth.organization_id)
        .bind(full_name)
        .bind(get_str("email"))
        .bind(get_str("phone"))
        .bind(get_str("id_number"))
        .bind(id)
        .bind(get_str("title"))
        .bind(get_str("customer_type").unwrap_or("individual"))
        .bind(get_str("kra_pin"))
        .bind(get_str("postal_address"))
        .bind(get_str("city"))
        .bind(get_str("physical_address"))
        .bind(legacy_number)
        .bind(get_str("next_of_kin_name"))
        .bind(get_str("next_of_kin_relationship"))
        .bind(get_str("next_of_kin_mobile"))
        .bind(get_str("next_of_kin_id_number"))
        .bind(get_str("next_of_kin_address"))
        .fetch_one(&mut *tx)
        .await?;

        sqlx::query("update migration_staging_rows set committed_entity_id = $1 where id = $2")
            .bind(customer_id)
            .bind(row.id)
            .execute(&mut *tx)
            .await?;

        committed += 1;
    }

    sqlx::query(
        "update migration_batches set status = 'committed', committed_at = now(), committed_rows = committed_rows + $1 where id = $2",
    )
    .bind(committed as i32)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(MigrationCommitResult { committed, skipped_exceptions, already_committed }))
}
