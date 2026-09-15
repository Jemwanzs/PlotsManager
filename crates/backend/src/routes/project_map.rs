//! Minimal interactive-map v1 — see `domain::ProjectMap`'s module docs
//! and `database/migrations/0009_project_map.sql` for the scope
//! decisions (one image + one polygon set per project, no
//! draft/pending-approval workflow).
//!
//! `image` is served from its own route rather than embedded as
//! base64 in the JSON metadata response, so a page that only needs
//! "does a map exist / what are the polygons" (e.g. deciding whether
//! to show an upload prompt) never pulls a multi-MB payload for it.
//! `<img>` tags can't send an `Authorization` header, so that route
//! authenticates via a `?token=` query param instead of the ordinary
//! `AuthUser` extractor.

use axum::body::Bytes;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, put};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use domain::{MapPolygons, ProjectMapSummary, UpdateMapPolygonsInput};
use uuid::Uuid;

use crate::auth::verify_session_token;
use crate::error::AppError;
use crate::extractors::AuthUser;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/projects/:id/map",
            get(get_map_summary).post(upload_map_image),
        )
        .route("/api/v1/projects/:id/map/image", get(get_map_image))
        .route("/api/v1/projects/:id/map/polygons", put(update_polygons))
}

async fn project_organization_id(
    db: &sqlx::PgPool,
    project_id: Uuid,
) -> Result<Option<Uuid>, AppError> {
    Ok(sqlx::query_scalar("select organization_id from projects where id = $1")
        .bind(project_id)
        .fetch_optional(db)
        .await?)
}

#[derive(sqlx::FromRow)]
struct MapMetaRow {
    image_content_type: String,
    polygons: serde_json::Value,
    updated_at: DateTime<Utc>,
}

async fn get_map_summary(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<Json<ProjectMapSummary>, AppError> {
    let Some(org_id) = project_organization_id(&state.db, project_id).await? else {
        return Err(AppError::NotFound);
    };
    if org_id != auth.organization_id {
        return Err(AppError::NotFound);
    }

    let row: Option<MapMetaRow> = sqlx::query_as(
        "select image_content_type, polygons, updated_at from project_maps where project_id = $1",
    )
    .bind(project_id)
    .fetch_optional(&state.db)
    .await?;

    Ok(Json(match row {
        Some(row) => ProjectMapSummary {
            exists: true,
            image_content_type: Some(row.image_content_type),
            polygons: serde_json::from_value(row.polygons).unwrap_or_default(),
            updated_at: Some(row.updated_at),
        },
        None => ProjectMapSummary {
            exists: false,
            image_content_type: None,
            polygons: MapPolygons::default(),
            updated_at: None,
        },
    }))
}

/// A fresh upload resets `polygons` to empty — pixel coordinates drawn
/// against the old image would almost never line up with a
/// differently-sized replacement, and silently keeping stale
/// coordinates is worse than making the admin redraw.
async fn upload_map_image(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(project_id): Path<Uuid>,
    mut multipart: Multipart,
) -> Result<Json<ProjectMapSummary>, AppError> {
    let Some(org_id) = project_organization_id(&state.db, project_id).await? else {
        return Err(AppError::NotFound);
    };
    if org_id != auth.organization_id {
        return Err(AppError::NotFound);
    }

    let mut image_data: Option<Bytes> = None;
    let mut content_type: Option<String> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::bad_request(format!("invalid upload: {e}")))?
    {
        if field.name() == Some("image") {
            content_type = field.content_type().map(str::to_string);
            image_data = Some(
                field
                    .bytes()
                    .await
                    .map_err(|e| AppError::bad_request(format!("invalid upload: {e}")))?,
            );
        }
    }
    let image_data = image_data.ok_or_else(|| AppError::bad_request("No image file provided."))?;
    let content_type = content_type.unwrap_or_else(|| "application/octet-stream".to_string());
    if !content_type.starts_with("image/") {
        return Err(AppError::bad_request("Only image files are accepted."));
    }
    if image_data.len() > 10 * 1024 * 1024 {
        return Err(AppError::bad_request("Image must be smaller than 10MB."));
    }

    sqlx::query(
        r#"
        insert into project_maps (project_id, organization_id, image_data, image_content_type, uploaded_by, polygons, updated_at)
        values ($1, $2, $3, $4, $5, '{"image_width": 0, "image_height": 0, "features": []}'::jsonb, now())
        on conflict (project_id) do update set
            image_data = excluded.image_data,
            image_content_type = excluded.image_content_type,
            uploaded_by = excluded.uploaded_by,
            polygons = excluded.polygons,
            updated_at = now()
        "#,
    )
    .bind(project_id)
    .bind(auth.organization_id)
    .bind(image_data.as_ref())
    .bind(&content_type)
    .bind(auth.user_id)
    .execute(&state.db)
    .await?;

    get_map_summary(State(state), auth, Path(project_id)).await
}

#[derive(serde::Deserialize)]
struct ImageQuery {
    token: String,
}

async fn get_map_image(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Query(params): Query<ImageQuery>,
) -> Result<Response, AppError> {
    let claims =
        verify_session_token(&params.token, &state.jwt_secret).map_err(|_| AppError::Unauthorized)?;

    let Some(org_id) = project_organization_id(&state.db, project_id).await? else {
        return Err(AppError::NotFound);
    };
    if org_id != claims.organization_id {
        return Err(AppError::NotFound);
    }

    let row: Option<(Vec<u8>, String)> = sqlx::query_as(
        "select image_data, image_content_type from project_maps where project_id = $1",
    )
    .bind(project_id)
    .fetch_optional(&state.db)
    .await?;
    let (data, content_type) = row.ok_or(AppError::NotFound)?;

    Ok((StatusCode::OK, [(header::CONTENT_TYPE, content_type)], data).into_response())
}

async fn update_polygons(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(project_id): Path<Uuid>,
    Json(input): Json<UpdateMapPolygonsInput>,
) -> Result<Json<ProjectMapSummary>, AppError> {
    let Some(org_id) = project_organization_id(&state.db, project_id).await? else {
        return Err(AppError::NotFound);
    };
    if org_id != auth.organization_id {
        return Err(AppError::NotFound);
    }

    for feature in &input.polygons.features {
        let plot_ok: bool = sqlx::query_scalar(
            "select exists(select 1 from plots where id = $1 and project_id = $2)",
        )
        .bind(feature.plot_id)
        .bind(project_id)
        .fetch_one(&state.db)
        .await?;
        if !plot_ok {
            return Err(AppError::bad_request(format!(
                "Plot {} doesn't belong to this project.",
                feature.plot_id
            )));
        }
    }

    let polygons_json = serde_json::to_value(&input.polygons)
        .map_err(|e| AppError::Internal(e.into()))?;

    let updated: bool = sqlx::query_scalar(
        "update project_maps set polygons = $1, updated_at = now() where project_id = $2 returning true",
    )
    .bind(polygons_json)
    .bind(project_id)
    .fetch_optional(&state.db)
    .await?
    .unwrap_or(false);
    if !updated {
        return Err(AppError::bad_request(
            "Upload a site plan image before saving plot boundaries.",
        ));
    }

    get_map_summary(State(state), auth, Path(project_id)).await
}
