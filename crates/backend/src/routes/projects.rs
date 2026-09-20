use axum::extract::Path;
use axum::{extract::State, routing::get, routing::post, routing::put, Json, Router};
use chrono::{DateTime, Utc};
use domain::{
    BulkImportResult, BulkImportRowError, CreatePlotInput, CreateProjectInput, Plot,
    PlotCommercialSummary, PlotLoanAccount, PlotSaleSummary, PlotWithColor, Project,
    ProjectSummary, UpdatePlotInput, PERM_PLOTS_BULK_IMPORT, PERM_PLOTS_CREATE, PERM_PLOTS_EDIT,
    PERM_PROJECTS_CREATE,
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
        .route(
            "/api/v1/projects/:project_id/plots/:plot_id/commercial-summary",
            get(get_plot_commercial_summary),
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

#[derive(sqlx::FromRow)]
struct PlotCommercialRow {
    // plot
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
    // sale (null when the plot has never had one)
    customer_id: Option<Uuid>,
    customer_name: Option<String>,
    payment_mode: Option<String>,
    agreed_price: Option<Decimal>,
    sale_created_at: Option<DateTime<Utc>>,
    // loan account (null for a full-cash sale, or no sale at all)
    loan_id: Option<Uuid>,
    account_number: Option<String>,
    sale_id: Option<Uuid>,
    principal: Option<Decimal>,
    interest_rate: Option<Decimal>,
    deposit_required: Option<Decimal>,
    deposit_paid: Option<Decimal>,
    instalment_amount: Option<Decimal>,
    repayment_frequency_days: Option<i32>,
    start_date: Option<chrono::NaiveDate>,
    loan_status: Option<String>,
    amount_paid: Option<Decimal>,
    outstanding_balance: Option<Decimal>,
    days_in_arrears: Option<i32>,
}

/// The full commercial position behind one plot — reservation/sale,
/// buyer, purchase type, and (for Lipa Pole Pole) the linked loan
/// account, all in one call. Only runs this join for the one plot
/// actually opened (see `PlotCommercialSummary`'s own docs on why
/// this isn't folded into `list_plots`).
async fn get_plot_commercial_summary(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((project_id, plot_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<PlotCommercialSummary>, AppError> {
    ensure_project_in_org(&state, project_id, auth.organization_id).await?;

    let row: Option<PlotCommercialRow> = sqlx::query_as(
        r#"
        select pl.id, pl.project_id, pl.plot_number, pl.title_number, pl.size, pl.side_1, pl.side_2,
            pl.dimension_unit, pl.asking_price, pl.minimum_price, pl.status, pl.map_feature_id,
            pl.assigned_customer_id, pl.created_at,
            ps.customer_id, c.full_name as customer_name, ps.payment_mode, ps.agreed_price,
            ps.created_at as sale_created_at,
            pla.id as loan_id, pla.account_number, pla.sale_id, pla.principal, pla.interest_rate,
            pla.deposit_required, pla.deposit_paid, pla.instalment_amount, pla.repayment_frequency_days,
            pla.start_date, pla.status as loan_status, pla.amount_paid, pla.outstanding_balance,
            pla.days_in_arrears
        from plots pl
        left join plot_sales ps on ps.id = (
            select id from plot_sales where plot_id = pl.id order by created_at desc limit 1
        )
        left join customers c on c.id = ps.customer_id
        left join plot_loan_accounts pla on pla.sale_id = ps.id
        where pl.id = $1 and pl.project_id = $2
        "#,
    )
    .bind(plot_id)
    .bind(project_id)
    .fetch_optional(&state.db)
    .await?;

    let row = row.ok_or(AppError::NotFound)?;

    let status: domain::PlotStatus = from_pg("plots.status", &row.status)?;
    let (status_label, status_color) = domain::plot_status_meta(status);

    let sale = match (row.customer_id, row.customer_name, row.payment_mode, row.agreed_price, row.sale_created_at) {
        (Some(customer_id), Some(customer_name), Some(payment_mode), Some(agreed_price), Some(created_at)) => {
            let loan_account = match (
                row.loan_id, row.account_number, row.sale_id, row.principal, row.deposit_required,
                row.deposit_paid, row.instalment_amount, row.repayment_frequency_days, row.start_date,
                row.loan_status.clone(), row.amount_paid, row.outstanding_balance, row.days_in_arrears,
            ) {
                (
                    Some(id), Some(account_number), Some(sale_id), Some(principal), Some(deposit_required),
                    Some(deposit_paid), Some(instalment_amount), Some(repayment_frequency_days), Some(start_date),
                    Some(loan_status_raw), Some(amount_paid), Some(outstanding_balance), Some(days_in_arrears),
                ) => Some(PlotLoanAccount {
                    id,
                    account_number,
                    sale_id,
                    principal,
                    interest_rate: row.interest_rate,
                    deposit_required,
                    deposit_paid,
                    instalment_amount,
                    repayment_frequency_days,
                    start_date,
                    status: from_pg("plot_loan_accounts.status", &loan_status_raw)?,
                    amount_paid,
                    outstanding_balance,
                    days_in_arrears,
                }),
                _ => None,
            };
            let (loan_status_label, loan_status_color) = match &loan_account {
                Some(l) => {
                    let (label, color) = domain::loan_status_meta(l.status);
                    (Some(label.to_string()), Some(color.to_string()))
                }
                None => (None, None),
            };

            Some(PlotSaleSummary {
                customer_id,
                customer_name,
                payment_mode: from_pg("plot_sales.payment_mode", &payment_mode)?,
                agreed_price,
                created_at,
                loan_account,
                loan_status_label,
                loan_status_color,
            })
        }
        _ => None,
    };

    Ok(Json(PlotCommercialSummary {
        plot: Plot {
            id: row.id,
            project_id: row.project_id,
            plot_number: row.plot_number,
            title_number: row.title_number,
            size: row.size,
            side_1: row.side_1,
            side_2: row.side_2,
            dimension_unit: row.dimension_unit,
            asking_price: row.asking_price,
            minimum_price: row.minimum_price,
            status,
            map_feature_id: row.map_feature_id,
            assigned_customer_id: row.assigned_customer_id,
            created_at: row.created_at,
        },
        status_label: status_label.to_string(),
        status_color: status_color.to_string(),
        sale,
    }))
}
