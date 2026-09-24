//! The generic document vault (legacy-migration-readiness gap
//! analysis, section 4-6) — see `domain::document`'s module docs for
//! the shape rationale. One table (`documents`), attachable to any of
//! six entity types via `(entity_type, entity_id)` rather than a
//! dedicated table/column per entity.
//!
//! Upload/delete require `documents:manage` (sensitive — replacing or
//! removing a KYC record, a title deed, or payment evidence has real
//! blast radius). Viewing (list + download) piggybacks on the caller
//! already having view access to the *parent* entity — a customer's
//! documents aren't visible to someone who can't view customers — via
//! `require_entity_view` below, rather than a separate
//! `documents:view` key that would need granting on top of every
//! existing view permission.
//!
//! Bytes are served from their own route (`GET .../file`), same split
//! as `project_map.rs`'s image, authenticated via `?token=` since
//! `<a href>`/`<img>` can't send an `Authorization` header.

use axum::body::Bytes;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use chrono::{DateTime, NaiveDate, Utc};
use domain::{DocumentEntityType, DocumentMeta, PERM_DOCUMENTS_MANAGE};
use uuid::Uuid;

use crate::auth::verify_session_token;
use crate::error::AppError;
use crate::extractors::AuthUser;
use crate::pg_enum::{from_pg, to_pg};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/documents",
            get(list_documents).post(upload_document),
        )
        .route("/api/v1/documents/:id", axum::routing::delete(delete_document))
        .route("/api/v1/documents/:id/file", get(get_document_file))
}

const MAX_FILE_BYTES: usize = 10 * 1024 * 1024;

fn allowed_mime(mime: &str) -> bool {
    matches!(mime, "application/pdf" | "image/jpeg" | "image/png")
}

/// Confirms the entity exists in the caller's org AND the caller has
/// view access to that entity type — the permission that gates
/// viewing/attaching documents on it. Returns the permission-checked
/// entity's own `organization_id` so callers don't re-derive it.
async fn require_entity_access(
    db: &sqlx::PgPool,
    auth: &AuthUser,
    entity_type: DocumentEntityType,
    entity_id: Uuid,
    require_view_permission: bool,
) -> Result<(), AppError> {
    if require_view_permission {
        let perm = match entity_type {
            DocumentEntityType::Customer => domain::PERM_CUSTOMERS_VIEW,
            DocumentEntityType::Plot | DocumentEntityType::Project | DocumentEntityType::Sale => {
                domain::PERM_PLOTS_VIEW
            }
            DocumentEntityType::LoanAccount | DocumentEntityType::Payment => domain::PERM_FINANCE_VIEW,
        };
        auth.require_permission(perm)?;
    }

    let org_id: Option<Uuid> = match entity_type {
        DocumentEntityType::Customer => {
            sqlx::query_scalar("select organization_id from customers where id = $1")
                .bind(entity_id)
                .fetch_optional(db)
                .await?
        }
        DocumentEntityType::Plot => {
            sqlx::query_scalar(
                "select p.organization_id from plots pl join projects p on p.id = pl.project_id where pl.id = $1",
            )
            .bind(entity_id)
            .fetch_optional(db)
            .await?
        }
        DocumentEntityType::Project => {
            sqlx::query_scalar("select organization_id from projects where id = $1")
                .bind(entity_id)
                .fetch_optional(db)
                .await?
        }
        DocumentEntityType::Sale => {
            sqlx::query_scalar("select organization_id from plot_sales where id = $1")
                .bind(entity_id)
                .fetch_optional(db)
                .await?
        }
        DocumentEntityType::LoanAccount => {
            sqlx::query_scalar(
                "select ps.organization_id from plot_loan_accounts pla join plot_sales ps on ps.id = pla.sale_id where pla.id = $1",
            )
            .bind(entity_id)
            .fetch_optional(db)
            .await?
        }
        DocumentEntityType::Payment => {
            sqlx::query_scalar(
                r#"select ps.organization_id from payments pay
                   join plot_loan_accounts pla on pla.id = pay.loan_account_id
                   join plot_sales ps on ps.id = pla.sale_id
                   where pay.id = $1"#,
            )
            .bind(entity_id)
            .fetch_optional(db)
            .await?
        }
    };

    match org_id {
        Some(id) if id == auth.organization_id => Ok(()),
        _ => Err(AppError::NotFound),
    }
}

#[derive(sqlx::FromRow)]
struct DocumentRow {
    id: Uuid,
    entity_type: String,
    entity_id: Uuid,
    document_type: String,
    document_number: Option<String>,
    original_filename: String,
    mime_type: String,
    file_size: i64,
    issue_date: Option<NaiveDate>,
    expiry_date: Option<NaiveDate>,
    description: Option<String>,
    uploaded_by_name: String,
    uploaded_at: DateTime<Utc>,
    legacy_source_path: Option<String>,
}

impl DocumentRow {
    fn into_domain(self) -> Result<DocumentMeta, AppError> {
        Ok(DocumentMeta {
            id: self.id,
            entity_type: from_pg("entity_type", &self.entity_type)?,
            entity_id: self.entity_id,
            document_type: self.document_type,
            document_number: self.document_number,
            original_filename: self.original_filename,
            mime_type: self.mime_type,
            file_size: self.file_size,
            issue_date: self.issue_date,
            expiry_date: self.expiry_date,
            description: self.description,
            uploaded_by_name: self.uploaded_by_name,
            uploaded_at: self.uploaded_at,
            legacy_source_path: self.legacy_source_path,
        })
    }
}

const DOCUMENT_COLUMNS: &str = "d.id, d.entity_type, d.entity_id, d.document_type, d.document_number,
    d.original_filename, d.mime_type, d.file_size, d.issue_date, d.expiry_date, d.description,
    u.full_name as uploaded_by_name, d.uploaded_at, d.legacy_source_path";

#[derive(serde::Deserialize)]
struct EntityQuery {
    entity_type: DocumentEntityType,
    entity_id: Uuid,
}

async fn list_documents(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<EntityQuery>,
) -> Result<Json<Vec<DocumentMeta>>, AppError> {
    require_entity_access(&state.db, &auth, q.entity_type, q.entity_id, true).await?;

    let rows: Vec<DocumentRow> = sqlx::query_as(&format!(
        "select {DOCUMENT_COLUMNS} from documents d
         join users u on u.id = d.uploaded_by
         where d.organization_id = $1 and d.entity_type = $2 and d.entity_id = $3
         order by d.uploaded_at desc"
    ))
    .bind(auth.organization_id)
    .bind(to_pg(&q.entity_type))
    .bind(q.entity_id)
    .fetch_all(&state.db)
    .await?;

    rows.into_iter().map(DocumentRow::into_domain).collect::<Result<Vec<_>, _>>().map(Json)
}

async fn upload_document(
    State(state): State<AppState>,
    auth: AuthUser,
    mut multipart: Multipart,
) -> Result<Json<DocumentMeta>, AppError> {
    auth.require_permission(PERM_DOCUMENTS_MANAGE)?;

    let mut entity_type: Option<DocumentEntityType> = None;
    let mut entity_id: Option<Uuid> = None;
    let mut document_type: Option<String> = None;
    let mut document_number: Option<String> = None;
    let mut issue_date: Option<NaiveDate> = None;
    let mut expiry_date: Option<NaiveDate> = None;
    let mut description: Option<String> = None;
    let mut file_data: Option<Bytes> = None;
    let mut mime_type: Option<String> = None;
    let mut original_filename: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::bad_request(format!("invalid upload: {e}")))?
    {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "file" => {
                original_filename = field.file_name().map(str::to_string);
                mime_type = field.content_type().map(str::to_string);
                file_data = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|e| AppError::bad_request(format!("invalid upload: {e}")))?,
                );
            }
            "entity_type" => {
                let text = field.text().await.unwrap_or_default();
                entity_type = Some(from_pg("entity_type", &text)?);
            }
            "entity_id" => {
                let text = field.text().await.unwrap_or_default();
                entity_id = Uuid::parse_str(&text).ok();
            }
            "document_type" => document_type = Some(field.text().await.unwrap_or_default()),
            "document_number" => {
                let text = field.text().await.unwrap_or_default();
                if !text.trim().is_empty() {
                    document_number = Some(text);
                }
            }
            "issue_date" => {
                let text = field.text().await.unwrap_or_default();
                issue_date = NaiveDate::parse_from_str(&text, "%Y-%m-%d").ok();
            }
            "expiry_date" => {
                let text = field.text().await.unwrap_or_default();
                expiry_date = NaiveDate::parse_from_str(&text, "%Y-%m-%d").ok();
            }
            "description" => {
                let text = field.text().await.unwrap_or_default();
                if !text.trim().is_empty() {
                    description = Some(text);
                }
            }
            _ => {}
        }
    }

    let entity_type = entity_type.ok_or_else(|| AppError::bad_request("Missing entity_type."))?;
    let entity_id = entity_id.ok_or_else(|| AppError::bad_request("Missing or invalid entity_id."))?;
    let document_type =
        document_type.filter(|s| !s.trim().is_empty()).ok_or_else(|| AppError::bad_request("Missing document_type."))?;
    let file_data = file_data.ok_or_else(|| AppError::bad_request("No file provided."))?;
    let original_filename = original_filename.unwrap_or_else(|| "document".to_string());
    let mime_type = mime_type.unwrap_or_else(|| "application/octet-stream".to_string());

    if !allowed_mime(&mime_type) {
        return Err(AppError::bad_request(
            "Only PDF, JPEG, or PNG files are accepted.",
        ));
    }
    if file_data.len() > MAX_FILE_BYTES {
        return Err(AppError::bad_request("File must be smaller than 10MB."));
    }
    if file_data.is_empty() {
        return Err(AppError::bad_request("The uploaded file is empty."));
    }

    require_entity_access(&state.db, &auth, entity_type, entity_id, false).await?;

    let row: DocumentRow = sqlx::query_as(&format!(
        r#"
        with inserted as (
            insert into documents (
                organization_id, entity_type, entity_id, document_type, document_number,
                original_filename, mime_type, file_size, file_data, issue_date, expiry_date,
                description, uploaded_by
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            returning *
        )
        select {DOCUMENT_COLUMNS} from inserted d join users u on u.id = d.uploaded_by
        "#,
    ))
    .bind(auth.organization_id)
    .bind(to_pg(&entity_type))
    .bind(entity_id)
    .bind(&document_type)
    .bind(&document_number)
    .bind(&original_filename)
    .bind(&mime_type)
    .bind(file_data.len() as i64)
    .bind(file_data.as_ref())
    .bind(issue_date)
    .bind(expiry_date)
    .bind(&description)
    .bind(auth.user_id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(row.into_domain()?))
}

async fn delete_document(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<()>, AppError> {
    auth.require_permission(PERM_DOCUMENTS_MANAGE)?;

    let result = sqlx::query("delete from documents where id = $1 and organization_id = $2")
        .bind(id)
        .bind(auth.organization_id)
        .execute(&state.db)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(Json(()))
}

#[derive(serde::Deserialize)]
struct FileQuery {
    token: String,
}

async fn get_document_file(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(params): Query<FileQuery>,
) -> Result<Response, AppError> {
    let claims =
        verify_session_token(&params.token, &state.jwt_secret).map_err(|_| AppError::Unauthorized)?;

    let row: Option<(Vec<u8>, String, String, Uuid)> = sqlx::query_as(
        "select file_data, mime_type, original_filename, organization_id from documents where id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;

    let Some((data, mime_type, filename, org_id)) = row else {
        return Err(AppError::NotFound);
    };
    if org_id != claims.organization_id {
        return Err(AppError::NotFound);
    }

    Ok((
        [
            (header::CONTENT_TYPE, mime_type),
            (
                header::CONTENT_DISPOSITION,
                format!("inline; filename=\"{}\"", filename.replace('"', "")),
            ),
        ],
        data,
    )
        .into_response())
}
