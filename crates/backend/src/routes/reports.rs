//! Three reports computed from data that already exists — no new
//! schema. See `domain::reports`'s module docs for why these three and
//! not the larger library docs/11-reports-and-analytics.md specifies
//! (most of the rest needs repayment-schedule/posted-payment
//! infrastructure that isn't built yet).

use axum::extract::Query;
use axum::routing::get;
use axum::{extract::State, Json, Router};
use chrono::{DateTime, NaiveDate, Utc};
use domain::{
    AgentPerformanceReport, AgentPerformanceRow, InventoryReport, PlotStatus, PlotStatusCount,
    ProjectInventoryRow, SalesReport, SalesReportRow,
};
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::error::AppError;
use crate::extractors::AuthUser;
use crate::pg_enum::from_pg;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/reports/sales", get(sales_report))
        .route("/api/v1/reports/inventory", get(inventory_report))
        .route("/api/v1/reports/agents", get(agent_performance_report))
}

#[derive(serde::Deserialize)]
struct SalesReportQuery {
    project_id: Option<Uuid>,
    agent_id: Option<Uuid>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
}

#[derive(sqlx::FromRow)]
struct SalesReportSqlRow {
    sale_id: Uuid,
    created_at: DateTime<Utc>,
    project_id: Uuid,
    project_name: String,
    plot_number: String,
    customer_id: Uuid,
    customer_name: String,
    agent_id: Option<Uuid>,
    agent_name: Option<String>,
    payment_mode: String,
    agreed_price: Decimal,
}

async fn sales_report(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(params): Query<SalesReportQuery>,
) -> Result<Json<SalesReport>, AppError> {
    let rows: Vec<SalesReportSqlRow> = sqlx::query_as(
        r#"
        select ps.id as sale_id, ps.created_at, p.id as project_id, p.name as project_name,
            pl.plot_number, c.id as customer_id, c.full_name as customer_name,
            ps.agent_id, u.full_name as agent_name, ps.payment_mode, ps.agreed_price
        from plot_sales ps
        join plots pl on pl.id = ps.plot_id
        join projects p on p.id = pl.project_id
        join customers c on c.id = ps.customer_id
        left join users u on u.id = ps.agent_id
        where ps.organization_id = $1
            and ($2::uuid is null or p.id = $2)
            and ($3::uuid is null or ps.agent_id = $3)
            and ($4::date is null or ps.created_at::date >= $4)
            and ($5::date is null or ps.created_at::date <= $5)
        order by ps.created_at desc
        "#,
    )
    .bind(auth.organization_id)
    .bind(params.project_id)
    .bind(params.agent_id)
    .bind(params.from)
    .bind(params.to)
    .fetch_all(&state.db)
    .await?;

    let mut total_value = Decimal::ZERO;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        total_value += row.agreed_price;
        out.push(SalesReportRow {
            sale_id: row.sale_id,
            created_at: row.created_at,
            project_id: row.project_id,
            project_name: row.project_name,
            plot_number: row.plot_number,
            customer_id: row.customer_id,
            customer_name: row.customer_name,
            agent_id: row.agent_id,
            agent_name: row.agent_name,
            payment_mode: from_pg("plot_sales.payment_mode", &row.payment_mode)?,
            agreed_price: row.agreed_price,
        });
    }
    let total_count = out.len() as u32;

    Ok(Json(SalesReport { rows: out, total_count, total_value }))
}

#[derive(sqlx::FromRow)]
struct InventorySqlRow {
    project_id: Uuid,
    project_name: String,
    status: String,
    count: i64,
    value: Decimal,
}

async fn inventory_report(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<InventoryReport>, AppError> {
    let rows: Vec<InventorySqlRow> = sqlx::query_as(
        r#"
        select p.id as project_id, p.name as project_name, pl.status,
            count(*) as count, coalesce(sum(pl.asking_price), 0) as value
        from projects p
        join plots pl on pl.project_id = p.id
        where p.organization_id = $1
        group by p.id, p.name, pl.status
        order by p.name
        "#,
    )
    .bind(auth.organization_id)
    .fetch_all(&state.db)
    .await?;

    let mut by_project: Vec<ProjectInventoryRow> = Vec::new();
    for row in rows {
        let status: PlotStatus = from_pg("plots.status", &row.status)?;
        let (label, color) = domain::plot_status_meta(status);
        let count = row.count as u32;
        let entry = PlotStatusCount {
            status,
            status_label: label.to_string(),
            status_color: color.to_string(),
            count,
            value: row.value,
        };

        match by_project.iter_mut().find(|p| p.project_id == row.project_id) {
            Some(project) => {
                project.total_plots += count;
                project.by_status.push(entry);
            }
            None => by_project.push(ProjectInventoryRow {
                project_id: row.project_id,
                project_name: row.project_name,
                total_plots: count,
                by_status: vec![entry],
            }),
        }
    }

    Ok(Json(InventoryReport { by_project }))
}

#[derive(serde::Deserialize)]
struct AgentReportQuery {
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
}

#[derive(sqlx::FromRow)]
struct AgentPerformanceSqlRow {
    agent_id: Uuid,
    agent_name: String,
    sales_count: i64,
    sales_value: Decimal,
    quotations_sent: i64,
    quotations_accepted: i64,
}

async fn agent_performance_report(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(params): Query<AgentReportQuery>,
) -> Result<Json<AgentPerformanceReport>, AppError> {
    let rows: Vec<AgentPerformanceSqlRow> = sqlx::query_as(
        r#"
        with sales_agg as (
            select agent_id, count(*) as sales_count, coalesce(sum(agreed_price), 0) as sales_value
            from plot_sales
            where organization_id = $1 and agent_id is not null
                and ($2::date is null or created_at::date >= $2)
                and ($3::date is null or created_at::date <= $3)
            group by agent_id
        ),
        quotes_agg as (
            -- 'sent' counts anything that has left Draft (sent, accepted or
            -- rejected all passed through Sent) — there's no status-history
            -- table to look this up precisely, so "reached Sent" is derived
            -- from the current status instead of stored separately.
            select agent_id,
                count(*) filter (where status in ('sent', 'accepted', 'rejected')) as sent,
                count(*) filter (where status = 'accepted') as accepted
            from quotations
            where organization_id = $1 and agent_id is not null
                and ($2::date is null or created_at::date >= $2)
                and ($3::date is null or created_at::date <= $3)
            group by agent_id
        )
        select coalesce(s.agent_id, q.agent_id) as agent_id, u.full_name as agent_name,
            coalesce(s.sales_count, 0) as sales_count, coalesce(s.sales_value, 0) as sales_value,
            coalesce(q.sent, 0) as quotations_sent, coalesce(q.accepted, 0) as quotations_accepted
        from sales_agg s
        full outer join quotes_agg q on q.agent_id = s.agent_id
        join users u on u.id = coalesce(s.agent_id, q.agent_id)
        order by sales_value desc
        "#,
    )
    .bind(auth.organization_id)
    .bind(params.from)
    .bind(params.to)
    .fetch_all(&state.db)
    .await?;

    let rows = rows
        .into_iter()
        .map(|r| AgentPerformanceRow {
            agent_id: r.agent_id,
            agent_name: r.agent_name,
            sales_count: r.sales_count as u32,
            sales_value: r.sales_value,
            quotations_sent: r.quotations_sent as u32,
            quotations_accepted: r.quotations_accepted as u32,
        })
        .collect();

    Ok(Json(AgentPerformanceReport { rows }))
}

