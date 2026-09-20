use axum::extract::Path;
use axum::{extract::State, routing::get, routing::post, routing::put, Json, Router};
use chrono::{DateTime, Utc};
use domain::{
    BulkImportResult, BulkImportRowError, CreatePlotInput, CreateProjectInput, Plot,
    PlotWithColor, Project, ProjectSummary, UpdatePlotInput, PERM_PLOTS_BULK_IMPORT,
    PERM_PLOTS_CREATE, PERM_PLOTS_EDIT, PERM_PROJECTS_CREATE,
};
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::error::AppError;
use crate::extractors::AuthUser;
use crate::pg_enum::{from_pg, to_pg};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/projects", get(list_projects).post(create_project))
        .route("/api/v1/projects/:id", get(get_project))
        .route(
            "/api/v1/projects/:id/plots",
            get(list_plots).post(create_plot),
        )
        .route("/api/v1/projects/:id/plots/bulk", post(bulk_create_plots))
        .route(
            "/api/v1/projects/:project_id/plots/:plot_id",
            put(update_plot),
        )
}

#[derive(sqlx::FromRow)]
struct ProjectSummaryRow {
    id: Uuid,
    name: String,
    code: String,
    location: String,
    status: String,
    total_plots: i64,
    available_plots: i64,
    sold_plots: i64,
}

async fn list_projects(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<ProjectSummary>>, AppError> {
    let rows: Vec<ProjectSummaryRow> = sqlx::query_as(
        r#"
        select p.id, p.name, p.code, p.location, p.status,
            count(pl.id) as total_plots,
            count(pl.id) filter (where pl.status = 'available') as available_plots,
            count(pl.id) filter (where pl.status = 'sold') as sold_plots
        from projects p
        left join plots pl on pl.project_id = p.id
        where p.organization_id = $1
        group by p.id
        order by p.created_at
        "#,
    )
    .bind(auth.organization_id)
    .fetch_all(&state.db)
    .await?;

    let summaries = rows
        .into_iter()
        .map(|r| {
            Ok(ProjectSummary {
                id: r.id,
                name: r.name,
                code: r.code,
                location: r.location,
                status: from_pg("projects.status", &r.status)?,
                total_plots: r.total_plots as u32,
                available_plots: r.available_plots as u32,
                sold_plots: r.sold_plots as u32,
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;

    Ok(Json(summaries))
}

#[derive(sqlx::FromRow)]
struct ProjectRow {
    id: Uuid,
    organization_id: Uuid,
    branch_id: Option<Uuid>,
    name: String,
    code: String,
    location: String,
    original_title_number: Option<String>,
    total_size: Decimal,
    area_unit: String,
    status: String,
    assigned_manager_id: Option<Uuid>,
    created_at: DateTime<Utc>,
}

impl ProjectRow {
    fn into_domain(self) -> Result<Project, AppError> {
        Ok(Project {
            id: self.id,
            organization_id: self.organization_id,
            branch_id: self.branch_id,
            name: self.name,
            code: self.code,
            location: self.location,
            original_title_number: self.original_title_number,
            total_size: self.total_size,
            area_unit: from_pg("projects.area_unit", &self.area_unit)?,
            status: from_pg("projects.status", &self.status)?,
            assigned_manager_id: self.assigned_manager_id,
            created_at: self.created_at,
        })
    }
}

async fn create_project(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<CreateProjectInput>,
) -> Result<Json<Project>, AppError> {
    auth.require_permission(PERM_PROJECTS_CREATE)?;

    let name = input.name.trim();
    let code = input.code.trim().to_uppercase();
    let location = input.location.trim();
    if name.is_empty() || code.is_empty() || location.is_empty() {
        return Err(AppError::bad_request(
            "Enter a project name, code, and location.",
        ));
    }

    let duplicate: bool = sqlx::query_scalar(
        "select exists(select 1 from projects where organization_id = $1 and code = $2)",
    )
    .bind(auth.organization_id)
    .bind(&code)
    .fetch_one(&state.db)
    .await?;
    if duplicate {
        return Err(AppError::conflict(format!(
            "Project code \"{code}\" is already in use."
        )));
    }

    let row: ProjectRow = sqlx::query_as(
        r#"
        insert into projects (organization_id, name, code, location, total_size, area_unit, status, assigned_manager_id)
        values ($1, $2, $3, $4, $5, $6, 'planning', $7)
        returning id, organization_id, branch_id, name, code, location, original_title_number,
            total_size, area_unit, status, assigned_manager_id, created_at
        "#,
    )
    .bind(auth.organization_id)
    .bind(name)
    .bind(&code)
    .bind(location)
    .bind(input.total_size)
    .bind(to_pg(&input.area_unit))
    .bind(auth.user_id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(row.into_domain()?))
}

async fn get_project(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Project>, AppError> {
    let row: Option<ProjectRow> = sqlx::query_as(
        r#"
        select id, organization_id, branch_id, name, code, location, original_title_number,
            total_size, area_unit, status, assigned_manager_id, created_at
        from projects where id = $1 and organization_id = $2
        "#,
    )
    .bind(id)
    .bind(auth.organization_id)
    .fetch_optional(&state.db)
    .await?;

    let row = row.ok_or(AppError::NotFound)?;
    Ok(Json(row.into_domain()?))
}

#[derive(sqlx::FromRow)]
struct PlotRow {
    id: Uuid,
    project_id: Uuid,
    plot_number: String,
    title_number: Option<String>,
    size: Decimal,
    side_1: Option<Decimal>,
    side_2: Option<Decimal>,
    dimension_unit: String,
    asking_price: Decimal,
    minimum_price: Decimal,
    status: String,
    map_feature_id: Option<String>,
    assigned_customer_id: Option<Uuid>,
    created_at: DateTime<Utc>,
}

const PLOT_COLUMNS: &str = "id, project_id, plot_number, title_number, size, side_1, side_2, \
    dimension_unit, asking_price, minimum_price, status, map_feature_id, assigned_customer_id, \
    created_at";

impl PlotRow {
    fn into_domain(self) -> Result<Plot, AppError> {
        Ok(Plot {
            id: self.id,
            project_id: self.project_id,
            plot_number: self.plot_number,
            title_number: self.title_number,
            size: self.size,
            side_1: self.side_1,
            side_2: self.side_2,
            dimension_unit: self.dimension_unit,
            asking_price: self.asking_price,
            minimum_price: self.minimum_price,
            status: from_pg("plots.status", &self.status)?,
            map_feature_id: self.map_feature_id,
            assigned_customer_id: self.assigned_customer_id,
            created_at: self.created_at,
        })
    }
}

async fn list_plots(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<PlotWithColor>>, AppError> {
    // Confirms the project belongs to the caller's org before returning
    // anything — otherwise a plot list for someone else's project id
    // would just come back empty, which reads as "no plots" instead of
    // the access-denial it actually is.
    ensure_project_in_org(&state, project_id, auth.organization_id).await?;

    let rows: Vec<PlotRow> = sqlx::query_as(&format!(
        "select {PLOT_COLUMNS} from plots where project_id = $1 order by plot_number"
    ))
    .bind(project_id)
    .fetch_all(&state.db)
    .await?;

    let plots = rows
        .into_iter()
        .map(|r| {
            let plot = r.into_domain()?;
            let (label, color) = domain::plot_status_meta(plot.status);
            Ok(PlotWithColor {
                plot,
                status_label: label.to_string(),
                status_color: color.to_string(),
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;

    Ok(Json(plots))
}

async fn create_plot(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(project_id): Path<Uuid>,
    Json(input): Json<CreatePlotInput>,
) -> Result<Json<Plot>, AppError> {
    auth.require_permission(PERM_PLOTS_CREATE)?;
    ensure_project_in_org(&state, project_id, auth.organization_id).await?;
    Ok(Json(insert_plot(&state, project_id, &input).await?))
}

/// The single-row validation-and-insert `create_plot` and
/// `bulk_create_plots` both go through, so a bulk CSV import can't
/// drift from what adding one plot by hand enforces. Does **not**
/// check the caller's org owns `project_id` — callers must do that
/// once up front (`ensure_project_in_org`), not per row.
pub(crate) async fn insert_plot(
    state: &AppState,
    project_id: Uuid,
    input: &CreatePlotInput,
) -> Result<Plot, AppError> {
    let plot_number = input.plot_number.trim();
    if plot_number.is_empty() {
        return Err(AppError::bad_request("Enter a plot number."));
    }
    if input.asking_price <= Decimal::ZERO {
        return Err(AppError::bad_request(
            "Enter an asking price greater than zero.",
        ));
    }
    if input.minimum_price > input.asking_price {
        return Err(AppError::bad_request(
            "Minimum price can't be higher than the asking price.",
        ));
    }
    validate_dimensions(input.side_1, input.side_2)?;

    let duplicate: bool = sqlx::query_scalar(
        "select exists(select 1 from plots where project_id = $1 and plot_number = $2)",
    )
    .bind(project_id)
    .bind(plot_number)
    .fetch_one(&state.db)
    .await?;
    if duplicate {
        return Err(AppError::conflict(format!(
            "Plot \"{plot_number}\" already exists in this project."
        )));
    }

    let row: PlotRow = sqlx::query_as(&format!(
        r#"
        insert into plots (project_id, plot_number, size, side_1, side_2, asking_price, minimum_price, status)
        values ($1, $2, $3, $4, $5, $6, $7, 'available')
        returning {PLOT_COLUMNS}
        "#,
    ))
    .bind(project_id)
    .bind(plot_number)
    .bind(input.size)
    .bind(input.side_1)
    .bind(input.side_2)
    .bind(input.asking_price)
    .bind(input.minimum_price)
    .fetch_one(&state.db)
    .await?;

    row.into_domain()
}

fn validate_dimensions(side_1: Option<Decimal>, side_2: Option<Decimal>) -> Result<(), AppError> {
    if side_1.is_some_and(|v| v <= Decimal::ZERO) || side_2.is_some_and(|v| v <= Decimal::ZERO) {
        return Err(AppError::bad_request(
            "Plot dimensions must be greater than zero.",
        ));
    }
    Ok(())
}

/// Edits a plot's own fields — `plot_number`, `size`, dimensions, and
/// pricing. Does not touch `status`, `assigned_customer_id`, or anything
/// the sales workflow owns; those change through their own endpoints
/// (`sales.rs`, `approvals.rs`), not here.
async fn update_plot(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((project_id, plot_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<UpdatePlotInput>,
) -> Result<Json<Plot>, AppError> {
    auth.require_permission(PERM_PLOTS_EDIT)?;
    ensure_project_in_org(&state, project_id, auth.organization_id).await?;

    let plot_number = input.plot_number.trim();
    if plot_number.is_empty() {
        return Err(AppError::bad_request("Enter a plot number."));
    }
    if input.asking_price <= Decimal::ZERO {
        return Err(AppError::bad_request(
            "Enter an asking price greater than zero.",
        ));
    }
    if input.minimum_price > input.asking_price {
        return Err(AppError::bad_request(
            "Minimum price can't be higher than the asking price.",
        ));
    }
    validate_dimensions(input.side_1, input.side_2)?;

    let duplicate: bool = sqlx::query_scalar(
        "select exists(select 1 from plots where project_id = $1 and plot_number = $2 and id <> $3)",
    )
    .bind(project_id)
    .bind(plot_number)
    .bind(plot_id)
    .fetch_one(&state.db)
    .await?;
    if duplicate {
        return Err(AppError::conflict(format!(
            "Plot \"{plot_number}\" already exists in this project."
        )));
    }

    let row: Option<PlotRow> = sqlx::query_as(&format!(
        r#"
        update plots
        set plot_number = $1, size = $2, side_1 = $3, side_2 = $4,
            asking_price = $5, minimum_price = $6
        where id = $7 and project_id = $8
        returning {PLOT_COLUMNS}
        "#,
    ))
    .bind(plot_number)
    .bind(input.size)
    .bind(input.side_1)
    .bind(input.side_2)
    .bind(input.asking_price)
    .bind(input.minimum_price)
    .bind(plot_id)
    .bind(project_id)
    .fetch_optional(&state.db)
    .await?;

    let row = row.ok_or(AppError::NotFound)?;
    Ok(Json(row.into_domain()?))
}

/// Best-effort bulk import (see `domain::BulkImportResult`'s module
/// docs) — a CSV upload during tenant onboarding, parsed to
/// `CreatePlotInput` rows client-side and posted here as JSON.
async fn bulk_create_plots(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(project_id): Path<Uuid>,
    Json(inputs): Json<Vec<CreatePlotInput>>,
) -> Result<Json<BulkImportResult>, AppError> {
    auth.require_permission(PERM_PLOTS_BULK_IMPORT)?;
    ensure_project_in_org(&state, project_id, auth.organization_id).await?;

    let mut created = 0u32;
    let mut errors = Vec::new();
    for (idx, input) in inputs.iter().enumerate() {
        match insert_plot(&state, project_id, input).await {
            Ok(_) => created += 1,
            Err(e) => errors.push(BulkImportRowError {
                row: idx as u32 + 1,
                message: e.client_message(),
            }),
        }
    }

    Ok(Json(BulkImportResult { created, errors }))
}

/// Shared by every plot route: 404s (not a bare permission error) if the
/// project doesn't belong to the caller's org, so a probing request can't
/// distinguish "doesn't exist" from "exists but isn't yours".
pub(crate) async fn ensure_project_in_org(
    state: &AppState,
    project_id: Uuid,
    organization_id: Uuid,
) -> Result<(), AppError> {
    let exists: bool = sqlx::query_scalar(
        "select exists(select 1 from projects where id = $1 and organization_id = $2)",
    )
    .bind(project_id)
    .bind(organization_id)
    .fetch_one(&state.db)
    .await?;
    if exists {
        Ok(())
    } else {
        Err(AppError::NotFound)
    }
}
