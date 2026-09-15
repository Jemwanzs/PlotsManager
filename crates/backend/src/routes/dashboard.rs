use axum::{extract::State, routing::get, Json, Router};
use domain::DashboardSummary;
use rust_decimal::Decimal;

use crate::error::AppError;
use crate::extractors::AuthUser;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/v1/dashboard", get(dashboard))
}

#[derive(sqlx::FromRow)]
struct DashboardRow {
    total_customers: i64,
    total_projects: i64,
    total_plots: i64,
    total_sales_count: i64,
    total_sales_value: Decimal,
    active_loans_count: i64,
    active_loan_book: Decimal,
    performing_count: i64,
    performing_amount: Decimal,
    non_performing_count: i64,
    non_performing_amount: Decimal,
}

/// Performing/non-performing is computed from real `plot_loan_accounts`
/// status, unlike `frontend::api::mock`'s synthetic 70/30 split (which
/// exists there only because the mock previously had no loan-account
/// statuses to aggregate over — the two should be reconciled once the
/// frontend is wired to this endpoint).
async fn dashboard(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<DashboardSummary>, AppError> {
    let row: DashboardRow = sqlx::query_as(
        r#"
        with org_plots as (
            select pl.* from plots pl
            join projects p on p.id = pl.project_id
            where p.organization_id = $1
        ),
        org_loan_accounts as (
            select pla.* from plot_loan_accounts pla
            join plot_sales ps on ps.id = pla.sale_id
            where ps.organization_id = $1
        )
        select
            (select count(*) from customers where organization_id = $1) as total_customers,
            (select count(*) from projects where organization_id = $1) as total_projects,
            (select count(*) from org_plots) as total_plots,
            (select count(*) from org_plots where status in ('sold', 'booked')) as total_sales_count,
            (select coalesce(sum(asking_price), 0) from org_plots where status in ('sold', 'booked')) as total_sales_value,
            (select count(*) from org_plots where status in ('booked', 'under_approval')) as active_loans_count,
            (select coalesce(sum(asking_price), 0) from org_plots where status in ('booked', 'under_approval')) as active_loan_book,
            (select count(*) from org_loan_accounts where status in ('active_current', 'active_partially_paid', 'in_grace_period')) as performing_count,
            (select coalesce(sum(outstanding_balance), 0) from org_loan_accounts where status in ('active_current', 'active_partially_paid', 'in_grace_period')) as performing_amount,
            (select count(*) from org_loan_accounts where status in ('in_arrears', 'defaulted', 'repossessed_or_reallocated')) as non_performing_count,
            (select coalesce(sum(outstanding_balance), 0) from org_loan_accounts where status in ('in_arrears', 'defaulted', 'repossessed_or_reallocated')) as non_performing_amount
        "#,
    )
    .bind(auth.organization_id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(DashboardSummary {
        total_customers: row.total_customers as u32,
        total_projects: row.total_projects as u32,
        total_plots: row.total_plots as u32,
        total_sales_count: row.total_sales_count as u32,
        total_sales_value: row.total_sales_value,
        active_loans_count: row.active_loans_count as u32,
        active_loan_book: row.active_loan_book,
        performing_count: row.performing_count as u32,
        performing_amount: row.performing_amount,
        non_performing_count: row.non_performing_count as u32,
        non_performing_amount: row.non_performing_amount,
    }))
}
