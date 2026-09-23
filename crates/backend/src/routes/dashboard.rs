use axum::{extract::State, routing::get, Json, Router};
use chrono::{DateTime, Datelike, Duration, TimeZone, Utc};
use domain::{DashboardAnalytics, DashboardSummary, MonthlySalesPoint, PlotStatusCount, ProjectSalesSlice};
use rust_decimal::Decimal;

use crate::error::AppError;
use crate::extractors::AuthUser;
use crate::pg_enum::from_pg;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/dashboard", get(dashboard))
        .route("/api/v1/dashboard/analytics", get(dashboard_analytics))
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

/// Performing/non-performing used to key off `plot_loan_accounts.status`
/// values (`in_arrears`, `defaulted`, ...) that nothing in this codebase
/// ever actually sets — `record_payment` only ever moves an account to
/// `active_partially_paid` or `fully_paid`, so every account looked
/// "performing" forever regardless of real payment history. It's keyed
/// off `loan_account_schedule_summary` instead now (see
/// 0022_repayment_schedule.sql): an account is non-performing exactly
/// when it has a schedule instalment overdue past the grace period,
/// recomputed fresh on every read rather than relying on a status flag
/// nothing keeps in sync.
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
            select pla.*, lass.oldest_overdue_due_date
            from plot_loan_accounts pla
            join plot_sales ps on ps.id = pla.sale_id
            left join loan_account_schedule_summary lass on lass.loan_account_id = pla.id
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
            (select count(*) from org_loan_accounts where status in ('active_current', 'active_partially_paid', 'in_grace_period') and oldest_overdue_due_date is null) as performing_count,
            (select coalesce(sum(outstanding_balance), 0) from org_loan_accounts where status in ('active_current', 'active_partially_paid', 'in_grace_period') and oldest_overdue_due_date is null) as performing_amount,
            (select count(*) from org_loan_accounts where status in ('active_current', 'active_partially_paid', 'in_grace_period', 'in_arrears', 'defaulted', 'repossessed_or_reallocated') and oldest_overdue_due_date is not null) as non_performing_count,
            (select coalesce(sum(outstanding_balance), 0) from org_loan_accounts where status in ('active_current', 'active_partially_paid', 'in_grace_period', 'in_arrears', 'defaulted', 'repossessed_or_reallocated') and oldest_overdue_due_date is not null) as non_performing_amount
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

#[derive(sqlx::FromRow)]
struct PeriodTotalsRow {
    qtd_value: Decimal,
    qtd_count: i64,
    ytd_value: Decimal,
    ytd_count: i64,
    prior_ytd_value: Decimal,
}

#[derive(sqlx::FromRow)]
struct MonthlyRow {
    month: DateTime<Utc>,
    value: Decimal,
    count: i64,
}

#[derive(sqlx::FromRow)]
struct StatusRow {
    status: String,
    count: i64,
    value: Decimal,
}

#[derive(sqlx::FromRow)]
struct ProjectSalesRow {
    project_name: String,
    value: Decimal,
    count: i64,
}

async fn dashboard_analytics(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<DashboardAnalytics>, AppError> {
    let now = Utc::now();
    let year_start = Utc
        .with_ymd_and_hms(now.year(), 1, 1, 0, 0, 0)
        .single()
        .unwrap_or(now);
    let quarter_start_month = ((now.month() - 1) / 3) * 3 + 1;
    let quarter_start = Utc
        .with_ymd_and_hms(now.year(), quarter_start_month, 1, 0, 0, 0)
        .single()
        .unwrap_or(now);
    // Same elapsed stretch of the previous year, so YTD has something
    // fair to compare against rather than a bare total.
    let prior_year_start = Utc
        .with_ymd_and_hms(now.year() - 1, 1, 1, 0, 0, 0)
        .single()
        .unwrap_or(year_start - Duration::days(365));
    let prior_year_asof = prior_year_start + (now - year_start);
    // First day of the month, 11 months back — trailing 12 months
    // including the current one. Exact calendar-month arithmetic
    // (not `now - Duration::days(335)`, which drifts depending on
    // which months fall in between).
    let months_back = now.year() * 12 + now.month0() as i32 - 11;
    let trend_start = Utc
        .with_ymd_and_hms(months_back.div_euclid(12), months_back.rem_euclid(12) as u32 + 1, 1, 0, 0, 0)
        .single()
        .unwrap_or(now);

    let totals: PeriodTotalsRow = sqlx::query_as(
        r#"
        select
            coalesce(sum(agreed_price) filter (where created_at >= $2), 0) as qtd_value,
            count(*) filter (where created_at >= $2) as qtd_count,
            coalesce(sum(agreed_price) filter (where created_at >= $3), 0) as ytd_value,
            count(*) filter (where created_at >= $3) as ytd_count,
            coalesce(sum(agreed_price) filter (where created_at >= $4 and created_at < $5), 0) as prior_ytd_value
        from plot_sales
        where organization_id = $1
        "#,
    )
    .bind(auth.organization_id)
    .bind(quarter_start)
    .bind(year_start)
    .bind(prior_year_start)
    .bind(prior_year_asof)
    .fetch_one(&state.db)
    .await?;

    let monthly_rows: Vec<MonthlyRow> = sqlx::query_as(
        r#"
        select date_trunc('month', created_at) as month,
            coalesce(sum(agreed_price), 0) as value, count(*) as count
        from plot_sales
        where organization_id = $1 and created_at >= $2
        group by month
        order by month
        "#,
    )
    .bind(auth.organization_id)
    .bind(trend_start)
    .fetch_all(&state.db)
    .await?;

    // Fill every one of the trailing 12 months even where the query
    // returned nothing, so the line chart shows a real zero instead of
    // silently skipping a month.
    let mut monthly_trend = Vec::with_capacity(12);
    for i in 0..12 {
        let month_start = add_months(trend_start, i);
        let found = monthly_rows
            .iter()
            .find(|r| r.month.year() == month_start.year() && r.month.month() == month_start.month());
        monthly_trend.push(MonthlySalesPoint {
            period_label: month_start.format("%b %Y").to_string(),
            sales_value: found.map(|r| r.value).unwrap_or(Decimal::ZERO),
            sales_count: found.map(|r| r.count as u32).unwrap_or(0),
        });
    }

    let status_rows: Vec<StatusRow> = sqlx::query_as(
        r#"
        select pl.status, count(*) as count, coalesce(sum(pl.asking_price), 0) as value
        from plots pl
        join projects p on p.id = pl.project_id
        where p.organization_id = $1
        group by pl.status
        "#,
    )
    .bind(auth.organization_id)
    .fetch_all(&state.db)
    .await?;

    let mut inventory_by_status = Vec::with_capacity(status_rows.len());
    for row in status_rows {
        let status = from_pg("plots.status", &row.status)?;
        let (label, color) = domain::plot_status_meta(status);
        inventory_by_status.push(PlotStatusCount {
            status,
            status_label: label.to_string(),
            status_color: color.to_string(),
            count: row.count as u32,
            value: row.value,
        });
    }

    let project_rows: Vec<ProjectSalesRow> = sqlx::query_as(
        r#"
        select p.name as project_name, coalesce(sum(ps.agreed_price), 0) as value, count(*) as count
        from plot_sales ps
        join plots pl on pl.id = ps.plot_id
        join projects p on p.id = pl.project_id
        where ps.organization_id = $1 and ps.created_at >= $2
        group by p.id, p.name
        order by value desc
        limit 8
        "#,
    )
    .bind(auth.organization_id)
    .bind(year_start)
    .fetch_all(&state.db)
    .await?;

    let sales_by_project = project_rows
        .into_iter()
        .map(|r| ProjectSalesSlice {
            project_name: r.project_name,
            sales_value: r.value,
            sales_count: r.count as u32,
        })
        .collect();

    Ok(Json(DashboardAnalytics {
        qtd_sales_value: totals.qtd_value,
        qtd_sales_count: totals.qtd_count as u32,
        ytd_sales_value: totals.ytd_value,
        ytd_sales_count: totals.ytd_count as u32,
        prior_ytd_sales_value: totals.prior_ytd_value,
        monthly_trend,
        inventory_by_status,
        sales_by_project,
    }))
}

/// First-of-month, `n` months after `start` (itself first-of-month) —
/// plain calendar month arithmetic that `chrono::Duration` (fixed-length
/// only) can't express directly.
fn add_months(start: DateTime<Utc>, n: u32) -> DateTime<Utc> {
    let total_months = (start.month0() as i32) + n as i32;
    let year = start.year() + total_months.div_euclid(12);
    let month = (total_months.rem_euclid(12)) as u32 + 1;
    Utc.with_ymd_and_hms(year, month, 1, 0, 0, 0).single().unwrap_or(start)
}
