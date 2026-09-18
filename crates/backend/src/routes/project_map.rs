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
//!
//! A drawn shape (`domain::MapFeature`) no longer requires a plot
//! picked up front — `update_polygons` (whole-set replace, used for
//! drawing/deleting shapes) now accepts an unlinked, `plot_id: None`
//! *draft* feature. `create_plot_for_feature`/`link_plot_to_feature`/
//! `unlink_feature` below are the single-feature actions that resolve
//! a draft (or undo that): fetch-modify-write against the same
//! `project_maps.polygons` JSONB column `update_polygons` writes to,
//! rather than a full array replace, so linking one shape can't race
//! with (or accidentally clobber) someone else mid-redraw.

use axum::body::Bytes;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, put};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use domain::{CreatePlotInput, LinkPlotInput, MapPolygons, ProjectMapSummary, UpdateMapPolygonsInput};
use uuid::Uuid;

use crate::auth::verify_session_token;
use crate::error::AppError;
use crate::extractors::AuthUser;
use crate::routes::projects::{ensure_project_in_org, insert_plot};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/projects/:id/map",
            get(get_map_summary).post(upload_map_image),
        )
        .route("/api/v1/projects/:id/map/image", get(get_map_image))
        .route("/api/v1/projects/:id/map/polygons", put(update_polygons))
        .route(
            "/api/v1/projects/:id/map/features/:feature_id/create-plot",
            put(create_plot_for_feature),
        )
        .route(
            "/api/v1/projects/:id/map/features/:feature_id/link-plot",
            put(link_plot_to_feature),
        )
        .route(
            "/api/v1/projects/:id/map/features/:feature_id/unlink",
            put(unlink_feature),
        )
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

    // A feature's `plot_id` is optional now (a freshly-drawn shape is a
    // draft until "Create Plot"/"Link Existing Plot" resolves it via
    // the single-feature endpoints below) — only a *linked* feature
    // needs its plot validated here, and at most one feature may claim
    // a given plot.
    let mut seen_plot_ids = std::collections::HashSet::new();
    for feature in &input.polygons.features {
        let Some(plot_id) = feature.plot_id else {
            continue;
        };
        if !seen_plot_ids.insert(plot_id) {
            return Err(AppError::bad_request(
                "Each plot can only be linked to one shape on the map.",
            ));
        }
        let plot_ok: bool = sqlx::query_scalar(
            "select exists(select 1 from plots where id = $1 and project_id = $2)",
        )
        .bind(plot_id)
        .bind(project_id)
        .fetch_one(&state.db)
        .await?;
        if !plot_ok {
            return Err(AppError::bad_request(format!(
                "Plot {plot_id} doesn't belong to this project."
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

async fn load_polygons(state: &AppState, project_id: Uuid) -> Result<MapPolygons, AppError> {
    let raw: Option<serde_json::Value> =
        sqlx::query_scalar("select polygons from project_maps where project_id = $1")
            .bind(project_id)
            .fetch_optional(&state.db)
            .await?;
    let raw = raw.ok_or_else(|| AppError::bad_request("Upload a site plan image first."))?;
    Ok(serde_json::from_value(raw).unwrap_or_default())
}

async fn save_polygons(
    state: &AppState,
    project_id: Uuid,
    polygons: &MapPolygons,
) -> Result<(), AppError> {
    let json = serde_json::to_value(polygons).map_err(|e| AppError::Internal(e.into()))?;
    sqlx::query("update project_maps set polygons = $1, updated_at = now() where project_id = $2")
        .bind(json)
        .bind(project_id)
        .execute(&state.db)
        .await?;
    Ok(())
}

/// "Create Plot" from a drawn-but-unlinked shape — reuses
/// `routes::projects::insert_plot` (the same validation/insert every
/// other plot-creation path goes through, including bulk import) so
/// this isn't a second plot-registration system, just a different
/// place to reach the first one from. Rejects a shape that's already
/// linked rather than silently creating an orphaned second plot.
async fn create_plot_for_feature(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((project_id, feature_id)): Path<(Uuid, String)>,
    Json(input): Json<CreatePlotInput>,
) -> Result<Json<ProjectMapSummary>, AppError> {
    ensure_project_in_org(&state, project_id, auth.organization_id).await?;

    let mut polygons = load_polygons(&state, project_id).await?;
    let feature = polygons
        .features
        .iter_mut()
        .find(|f| f.id == feature_id)
        .ok_or(AppError::NotFound)?;
    if feature.plot_id.is_some() {
        return Err(AppError::conflict(
            "This shape is already linked to a plot.",
        ));
    }

    let plot = insert_plot(&state, project_id, &input).await?;
    feature.plot_id = Some(plot.id);
    save_polygons(&state, project_id, &polygons).await?;

    get_map_summary(State(state), auth, Path(project_id)).await
}

/// "Link Existing Plot" — attaches an already-registered, not-yet-
/// mapped plot to a drawn shape.
async fn link_plot_to_feature(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((project_id, feature_id)): Path<(Uuid, String)>,
    Json(input): Json<LinkPlotInput>,
) -> Result<Json<ProjectMapSummary>, AppError> {
    ensure_project_in_org(&state, project_id, auth.organization_id).await?;

    let plot_ok: bool = sqlx::query_scalar(
        "select exists(select 1 from plots where id = $1 and project_id = $2)",
    )
    .bind(input.plot_id)
    .bind(project_id)
    .fetch_one(&state.db)
    .await?;
    if !plot_ok {
        return Err(AppError::bad_request(
            "That plot doesn't belong to this project.",
        ));
    }

    let mut polygons = load_polygons(&state, project_id).await?;
    if polygons
        .features
        .iter()
        .any(|f| f.plot_id == Some(input.plot_id))
    {
        return Err(AppError::conflict(
            "That plot is already linked to a shape on this map.",
        ));
    }
    let feature = polygons
        .features
        .iter_mut()
        .find(|f| f.id == feature_id)
        .ok_or(AppError::NotFound)?;
    if feature.plot_id.is_some() {
        return Err(AppError::conflict(
            "This shape is already linked to a plot.",
        ));
    }
    feature.plot_id = Some(input.plot_id);
    save_polygons(&state, project_id, &polygons).await?;

    get_map_summary(State(state), auth, Path(project_id)).await
}

/// Detaches a shape from its plot without deleting either — the plot
/// record stays exactly as it was, just no longer mapped. (Undoing a
/// mis-click, not a plot deletion path.)
async fn unlink_feature(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((project_id, feature_id)): Path<(Uuid, String)>,
) -> Result<Json<ProjectMapSummary>, AppError> {
    ensure_project_in_org(&state, project_id, auth.organization_id).await?;

    let mut polygons = load_polygons(&state, project_id).await?;
    let feature = polygons
        .features
        .iter_mut()
        .find(|f| f.id == feature_id)
        .ok_or(AppError::NotFound)?;
    feature.plot_id = None;
    save_polygons(&state, project_id, &polygons).await?;

    get_map_summary(State(state), auth, Path(project_id)).await
}
