//! Interactive map with real versioning (`domain::ProjectMapVersion`'s
//! module docs; `database/migrations/0038_project_map_versions.sql`
//! for the schema) — replaces the single-mutable-row v1 slice
//! (`0009_project_map.sql`). At most one `draft` and one `published`
//! row exist per project at a time: edit-mode actions (draw/delete a
//! shape, "Save map", create/link/unlink a plot) all target the draft;
//! everything else (plot selection, sale initiation) reads the
//! published version. "Publish" promotes the draft; whatever was
//! published before becomes `superseded`, permanent read-only history.
//!
//! `image` is served from its own route rather than embedded as
//! base64 in the JSON metadata response, so a page that only needs
//! "does a map exist / what are the polygons" (e.g. deciding whether
//! to show an upload prompt) never pulls a multi-MB payload for it.
//! `<img>` tags can't send an `Authorization` header, so that route
//! authenticates via a `?token=` query param instead of the ordinary
//! `AuthUser` extractor — now keyed by a specific version's id, since
//! more than one version's image can exist for a project at once.
//!
//! A drawn shape (`domain::MapFeature`) doesn't require a plot picked
//! up front — `update_polygons` (whole-set replace, used for drawing/
//! deleting shapes) accepts an unlinked, `plot_id: None` *draft*
//! feature. `create_plot_for_feature`/`link_plot_to_feature`/
//! `unlink_feature` below are the single-feature actions that resolve
//! a draft (or undo that): fetch-modify-write against the current
//! draft version's `polygons` JSONB column rather than a full array
//! replace, so linking one shape can't race with (or accidentally
//! clobber) someone else mid-redraw.

use axum::body::Bytes;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use domain::{
    CreatePlotInput, LinkPlotInput, MapPolygons, ProjectMapSummary, ProjectMapVersion,
    UpdateMapPolygonsInput, PERM_PLOTS_MAP_EDIT_BOUNDARIES, PERM_PLOTS_MAP_LINK,
    PERM_PLOTS_MAP_UPLOAD,
};
use uuid::Uuid;

use crate::auth::verify_session_token;
use crate::error::AppError;
use crate::extractors::AuthUser;
use crate::pg_enum::from_pg;
use crate::routes::projects::{ensure_project_in_org, insert_plot};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/projects/:id/map",
            get(get_map_summary).post(upload_map_image),
        )
        .route("/api/v1/projects/:id/map/draft", post(ensure_draft).delete(discard_draft))
        .route("/api/v1/projects/:id/map/publish", post(publish_draft))
        .route("/api/v1/projects/:id/map/versions", get(list_versions))
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
struct MapVersionRow {
    id: Uuid,
    version_number: i32,
    status: String,
    image_content_type: String,
    polygons: serde_json::Value,
    created_by_name: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    published_by_name: Option<String>,
    published_at: Option<DateTime<Utc>>,
}

impl MapVersionRow {
    fn into_domain(self) -> Result<ProjectMapVersion, AppError> {
        Ok(ProjectMapVersion {
            id: self.id,
            version_number: self.version_number,
            status: from_pg("project_map_versions.status", &self.status)?,
            image_content_type: self.image_content_type,
            polygons: serde_json::from_value(self.polygons).unwrap_or_default(),
            created_by_name: self.created_by_name,
            created_at: self.created_at,
            updated_at: self.updated_at,
            published_by_name: self.published_by_name,
            published_at: self.published_at,
        })
    }
}

const MAP_VERSION_QUERY: &str = r#"
    select v.id, v.version_number, v.status, v.image_content_type, v.polygons,
        creator.full_name as created_by_name, v.created_at, v.updated_at,
        publisher.full_name as published_by_name, v.published_at
    from project_map_versions v
    join users creator on creator.id = v.created_by
    left join users publisher on publisher.id = v.published_by
"#;

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

    let rows: Vec<MapVersionRow> = sqlx::query_as(&format!(
        "{MAP_VERSION_QUERY} where v.project_id = $1 and v.status in ('draft', 'published')"
    ))
    .bind(project_id)
    .fetch_all(&state.db)
    .await?;

    let mut published = None;
    let mut draft = None;
    for row in rows {
        match row.status.as_str() {
            "published" => published = Some(row.into_domain()?),
            _ => draft = Some(row.into_domain()?),
        }
    }

    Ok(Json(ProjectMapSummary { published, draft }))
}

/// A fresh upload resets `polygons` to empty — pixel coordinates drawn
/// against the old image would almost never line up with a
/// differently-sized replacement, and silently keeping stale
/// coordinates is worse than making the admin redraw. Creates the
/// draft if none exists yet (first upload, or the first edit after a
/// publish); replaces the existing draft's image otherwise. Never
/// touches the published version — that's exactly the point of the
/// draft/publish split.
async fn upload_map_image(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(project_id): Path<Uuid>,
    mut multipart: Multipart,
) -> Result<Json<ProjectMapSummary>, AppError> {
    auth.require_permission(PERM_PLOTS_MAP_UPLOAD)?;

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

    let empty_polygons = serde_json::json!({"image_width": 0, "image_height": 0, "features": []});

    let draft_id: Option<Uuid> =
        sqlx::query_scalar("select id from project_map_versions where project_id = $1 and status = 'draft'")
            .bind(project_id)
            .fetch_optional(&state.db)
            .await?;

    match draft_id {
        Some(id) => {
            sqlx::query(
                "update project_map_versions set image_data = $1, image_content_type = $2, \
                 polygons = $3, updated_at = now() where id = $4",
            )
            .bind(image_data.as_ref())
            .bind(&content_type)
            .bind(&empty_polygons)
            .bind(id)
            .execute(&state.db)
            .await?;
        }
        None => {
            let next_version: i32 = sqlx::query_scalar(
                "select coalesce(max(version_number), 0) + 1 from project_map_versions where project_id = $1",
            )
            .bind(project_id)
            .fetch_one(&state.db)
            .await?;
            sqlx::query(
                "insert into project_map_versions \
                 (project_id, organization_id, version_number, status, image_data, image_content_type, polygons, created_by) \
                 values ($1, $2, $3, 'draft', $4, $5, $6, $7)",
            )
            .bind(project_id)
            .bind(auth.organization_id)
            .bind(next_version)
            .bind(image_data.as_ref())
            .bind(&content_type)
            .bind(&empty_polygons)
            .bind(auth.user_id)
            .execute(&state.db)
            .await?;
        }
    }

    get_map_summary(State(state), auth, Path(project_id)).await
}

/// Explicit "start editing": clones the published version's image and
/// polygons into a new draft row. No-op if a draft already exists
/// (resuming an edit in progress). 400s if neither a draft nor a
/// published version exists yet — the frontend only reaches this from
/// inside `MapCanvas`, which doesn't render until at least one of them
/// does (see `pages/project_detail.rs::ProjectMapSection`).
async fn ensure_draft(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<Json<ProjectMapSummary>, AppError> {
    auth.require_permission(PERM_PLOTS_MAP_EDIT_BOUNDARIES)?;
    ensure_project_in_org(&state, project_id, auth.organization_id).await?;

    let draft_exists: bool = sqlx::query_scalar(
        "select exists(select 1 from project_map_versions where project_id = $1 and status = 'draft')",
    )
    .bind(project_id)
    .fetch_one(&state.db)
    .await?;

    if !draft_exists {
        #[derive(sqlx::FromRow)]
        struct Published {
            image_data: Vec<u8>,
            image_content_type: String,
            polygons: serde_json::Value,
        }
        let published: Option<Published> = sqlx::query_as(
            "select image_data, image_content_type, polygons from project_map_versions \
             where project_id = $1 and status = 'published'",
        )
        .bind(project_id)
        .fetch_optional(&state.db)
        .await?;
        let published = published.ok_or_else(|| {
            AppError::bad_request("Upload a site plan image before editing boundaries.")
        })?;

        let next_version: i32 = sqlx::query_scalar(
            "select coalesce(max(version_number), 0) + 1 from project_map_versions where project_id = $1",
        )
        .bind(project_id)
        .fetch_one(&state.db)
        .await?;
        sqlx::query(
            "insert into project_map_versions \
             (project_id, organization_id, version_number, status, image_data, image_content_type, polygons, created_by) \
             values ($1, $2, $3, 'draft', $4, $5, $6, $7)",
        )
        .bind(project_id)
        .bind(auth.organization_id)
        .bind(next_version)
        .bind(&published.image_data)
        .bind(&published.image_content_type)
        .bind(&published.polygons)
        .bind(auth.user_id)
        .execute(&state.db)
        .await?;
    }

    get_map_summary(State(state), auth, Path(project_id)).await
}

/// Discards the current draft outright — the undo path a real draft
/// needs. Reverts to showing the published version (or the upload
/// prompt, if this project has never published one).
async fn discard_draft(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<Json<ProjectMapSummary>, AppError> {
    auth.require_permission(PERM_PLOTS_MAP_EDIT_BOUNDARIES)?;
    ensure_project_in_org(&state, project_id, auth.organization_id).await?;

    let deleted: bool = sqlx::query_scalar(
        "delete from project_map_versions where project_id = $1 and status = 'draft' returning true",
    )
    .bind(project_id)
    .fetch_optional(&state.db)
    .await?
    .unwrap_or(false);
    if !deleted {
        return Err(AppError::bad_request("There's no draft to discard."));
    }

    get_map_summary(State(state), auth, Path(project_id)).await
}

/// Promotes the current draft to published; whatever was published
/// before becomes permanent, read-only `superseded` history. Both
/// updates happen in one transaction so a crash mid-publish can't
/// leave the project with either zero or two published versions.
async fn publish_draft(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<Json<ProjectMapSummary>, AppError> {
    auth.require_permission(PERM_PLOTS_MAP_EDIT_BOUNDARIES)?;
    ensure_project_in_org(&state, project_id, auth.organization_id).await?;

    let mut tx = state.db.begin().await?;

    let draft_id: Option<Uuid> =
        sqlx::query_scalar("select id from project_map_versions where project_id = $1 and status = 'draft'")
            .bind(project_id)
            .fetch_optional(&mut *tx)
            .await?;
    let draft_id = draft_id.ok_or_else(|| AppError::bad_request("There's no draft to publish."))?;

    sqlx::query(
        "update project_map_versions set status = 'superseded' where project_id = $1 and status = 'published'",
    )
    .bind(project_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "update project_map_versions set status = 'published', published_by = $1, published_at = now(), \
         updated_at = now() where id = $2",
    )
    .bind(auth.user_id)
    .bind(draft_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    get_map_summary(State(state), auth, Path(project_id)).await
}

/// Full version history, newest first — metadata only (no image
/// bytes), same avoidance `ProjectMapSummary` already applies.
async fn list_versions(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<ProjectMapVersion>>, AppError> {
    let Some(org_id) = project_organization_id(&state.db, project_id).await? else {
        return Err(AppError::NotFound);
    };
    if org_id != auth.organization_id {
        return Err(AppError::NotFound);
    }

    let rows: Vec<MapVersionRow> = sqlx::query_as(&format!(
        "{MAP_VERSION_QUERY} where v.project_id = $1 order by v.version_number desc"
    ))
    .bind(project_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(MapVersionRow::into_domain)
            .collect::<Result<Vec<_>, AppError>>()?,
    ))
}

#[derive(serde::Deserialize)]
struct ImageQuery {
    token: String,
    version_id: Uuid,
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
        "select image_data, image_content_type from project_map_versions where id = $1 and project_id = $2",
    )
    .bind(params.version_id)
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
    auth.require_permission(PERM_PLOTS_MAP_EDIT_BOUNDARIES)?;

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
        "update project_map_versions set polygons = $1, updated_at = now() \
         where project_id = $2 and status = 'draft' returning true",
    )
    .bind(polygons_json)
    .bind(project_id)
    .fetch_optional(&state.db)
    .await?
    .unwrap_or(false);
    if !updated {
        return Err(AppError::bad_request(
            "Start editing boundaries before saving plot shapes.",
        ));
    }

    get_map_summary(State(state), auth, Path(project_id)).await
}

async fn load_draft_polygons(state: &AppState, project_id: Uuid) -> Result<MapPolygons, AppError> {
    let raw: Option<serde_json::Value> = sqlx::query_scalar(
        "select polygons from project_map_versions where project_id = $1 and status = 'draft'",
    )
    .bind(project_id)
    .fetch_optional(&state.db)
    .await?;
    let raw = raw.ok_or_else(|| AppError::bad_request("Start editing boundaries first."))?;
    Ok(serde_json::from_value(raw).unwrap_or_default())
}

async fn save_draft_polygons(
    state: &AppState,
    project_id: Uuid,
    polygons: &MapPolygons,
) -> Result<(), AppError> {
    let json = serde_json::to_value(polygons).map_err(|e| AppError::Internal(e.into()))?;
    sqlx::query(
        "update project_map_versions set polygons = $1, updated_at = now() \
         where project_id = $2 and status = 'draft'",
    )
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
/// Always resolves against the draft — see this module's own docs on
/// why edit-mode actions never touch the published version directly.
async fn create_plot_for_feature(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((project_id, feature_id)): Path<(Uuid, String)>,
    Json(input): Json<CreatePlotInput>,
) -> Result<Json<ProjectMapSummary>, AppError> {
    auth.require_permission(PERM_PLOTS_MAP_LINK)?;
    ensure_project_in_org(&state, project_id, auth.organization_id).await?;

    let mut polygons = load_draft_polygons(&state, project_id).await?;
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
    save_draft_polygons(&state, project_id, &polygons).await?;

    get_map_summary(State(state), auth, Path(project_id)).await
}

/// "Link Existing Plot" — attaches an already-registered, not-yet-
/// mapped plot to a drawn shape on the draft.
async fn link_plot_to_feature(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((project_id, feature_id)): Path<(Uuid, String)>,
    Json(input): Json<LinkPlotInput>,
) -> Result<Json<ProjectMapSummary>, AppError> {
    auth.require_permission(PERM_PLOTS_MAP_LINK)?;
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

    let mut polygons = load_draft_polygons(&state, project_id).await?;
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
    save_draft_polygons(&state, project_id, &polygons).await?;

    get_map_summary(State(state), auth, Path(project_id)).await
}

/// Detaches a shape from its plot without deleting either — the plot
/// record stays exactly as it was, just no longer mapped on the draft.
/// (Undoing a mis-click, not a plot deletion path.)
async fn unlink_feature(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((project_id, feature_id)): Path<(Uuid, String)>,
) -> Result<Json<ProjectMapSummary>, AppError> {
    auth.require_permission(PERM_PLOTS_MAP_LINK)?;
    ensure_project_in_org(&state, project_id, auth.organization_id).await?;

    let mut polygons = load_draft_polygons(&state, project_id).await?;
    let feature = polygons
        .features
        .iter_mut()
        .find(|f| f.id == feature_id)
        .ok_or(AppError::NotFound)?;
    feature.plot_id = None;
    save_draft_polygons(&state, project_id, &polygons).await?;

    get_map_summary(State(state), auth, Path(project_id)).await
}
