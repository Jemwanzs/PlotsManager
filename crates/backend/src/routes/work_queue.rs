//! `GET /api/v1/work-queue` — see `domain::work_queue`'s module docs.
//! Every section below is gated behind the same view permission that
//! already guards the full report/list it summarizes (a caller who
//! can't see loan accounts doesn't get an arrears item either), so
//! this never surfaces something the requester couldn't otherwise see
//! — it isn't a new visibility boundary, just a faster way to reach
//! what's already reachable.

use axum::{extract::State, routing::get, Json, Router};
use chrono::{NaiveDate, Utc};
use domain::{
    WorkQueue, WorkQueueItem, WorkQueueItemKind, PERM_APPROVALS_VIEW, PERM_CUSTOMERS_VIEW,
    PERM_FINANCE_VIEW, PERM_QUOTES_VIEW,
};
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::error::AppError;
use crate::extractors::AuthUser;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/v1/work-queue", get(get_work_queue))
}

/// How far ahead "expiring soon" / "due soon" looks — a quotation
/// valid for another three weeks isn't yet anyone's problem; one
/// expiring tomorrow is. Same window for both quotations and lead
/// follow-ups, matching the loan schedule's own grace-period-style
/// literal-constant precedent (`0022_repayment_schedule.sql`) rather
/// than inventing a separate config surface for this first cut.
const LOOKAHEAD_DAYS: i64 = 7;

async fn get_work_queue(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<WorkQueue>, AppError> {
    let today = Utc::now().date_naive();
    let mut items = Vec::new();

    if auth.has_permission(PERM_FINANCE_VIEW) {
        items.extend(arrears_items(&state, auth.organization_id).await?);
    }
    if auth.has_permission(PERM_QUOTES_VIEW) {
        items.extend(expiring_quotation_items(&state, auth.organization_id, today).await?);
    }
    if auth.has_permission(PERM_APPROVALS_VIEW) {
        items.extend(pending_approval_items(&state, auth.organization_id).await?);
    }
    if auth.has_permission(PERM_CUSTOMERS_VIEW) {
        items.extend(lead_follow_up_items(&state, auth.organization_id, today).await?);
    }
    if auth.is_platform_owner {
        items.extend(tenant_pending_items(&state).await?);
    }

    // Overdue first (most days overdue leads), then soonest-due, then
    // whatever has neither (approvals pending have no date of their
    // own) — a flat "everything urgent floats up" ordering rather than
    // grouping by kind, since a wildly overdue loan matters more than
    // a quotation expiring next week regardless of category.
    items.sort_by(|a, b| {
        let a_key = (
            std::cmp::Reverse(a.days_overdue.unwrap_or(0).max(0)),
            a.due_date.unwrap_or(NaiveDate::MAX),
        );
        let b_key = (
            std::cmp::Reverse(b.days_overdue.unwrap_or(0).max(0)),
            b.due_date.unwrap_or(NaiveDate::MAX),
        );
        a_key.cmp(&b_key)
    });

    Ok(Json(WorkQueue { items }))
}

#[derive(sqlx::FromRow)]
struct ArrearsRow {
    id: Uuid,
    account_number: String,
    customer_name: String,
    days_in_arrears: i32,
    outstanding_balance: Decimal,
}

async fn arrears_items(state: &AppState, org_id: Uuid) -> Result<Vec<WorkQueueItem>, AppError> {
    let rows: Vec<ArrearsRow> = sqlx::query_as(
        r#"
        select pla.id, pla.account_number, c.full_name as customer_name,
            (current_date - lass.oldest_overdue_due_date)::int as days_in_arrears,
            pla.outstanding_balance
        from plot_loan_accounts pla
        join plot_sales ps on ps.id = pla.sale_id
        join customers c on c.id = ps.customer_id
        join loan_account_schedule_summary lass on lass.loan_account_id = pla.id
        where ps.organization_id = $1 and lass.oldest_overdue_due_date is not null
        order by days_in_arrears desc
        limit 50
        "#,
    )
    .bind(org_id)
    .fetch_all(&state.db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| WorkQueueItem {
            kind: WorkQueueItemKind::LoanArrears,
            title: format!("{} — {}", r.account_number, r.customer_name),
            subtitle: format!("{} day(s) overdue", r.days_in_arrears),
            amount: Some(r.outstanding_balance),
            href: format!("/loan-accounts/{}", r.id),
            due_date: None,
            days_overdue: Some(r.days_in_arrears),
        })
        .collect())
}

#[derive(sqlx::FromRow)]
struct ExpiringQuotationRow {
    id: Uuid,
    plot_number: String,
    customer_name: String,
    quoted_price: Decimal,
    valid_until: NaiveDate,
}

async fn expiring_quotation_items(
    state: &AppState,
    org_id: Uuid,
    today: NaiveDate,
) -> Result<Vec<WorkQueueItem>, AppError> {
    let rows: Vec<ExpiringQuotationRow> = sqlx::query_as(
        r#"
        select q.id, pl.plot_number, c.full_name as customer_name, q.quoted_price, q.valid_until
        from quotations q
        join plots pl on pl.id = q.plot_id
        join customers c on c.id = q.customer_id
        where q.organization_id = $1 and q.status = 'sent'
            and q.valid_until between $2 and $2 + $3
        order by q.valid_until
        limit 50
        "#,
    )
    .bind(org_id)
    .bind(today)
    .bind(LOOKAHEAD_DAYS as i32)
    .fetch_all(&state.db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| {
            let days_left = (r.valid_until - today).num_days();
            WorkQueueItem {
                kind: WorkQueueItemKind::QuotationExpiring,
                title: format!("{} — {}", r.plot_number, r.customer_name),
                subtitle: if days_left <= 0 {
                    "Expires today".to_string()
                } else {
                    format!("Expires in {days_left} day(s)")
                },
                amount: Some(r.quoted_price),
                href: format!("/quotations/{}", r.id),
                due_date: Some(r.valid_until),
                days_overdue: None,
            }
        })
        .collect())
}

#[derive(sqlx::FromRow)]
struct PendingApprovalRow {
    plot_number: String,
    customer_name: String,
    agreed_price: Decimal,
    requested_by_name: String,
}

async fn pending_approval_items(state: &AppState, org_id: Uuid) -> Result<Vec<WorkQueueItem>, AppError> {
    let rows: Vec<PendingApprovalRow> = sqlx::query_as(
        r#"
        select pl.plot_number, c.full_name as customer_name, ar.agreed_price,
            u.full_name as requested_by_name
        from approval_requests ar
        join plots pl on pl.id = ar.plot_id
        join customers c on c.id = ar.customer_id
        join users u on u.id = ar.requested_by
        where ar.organization_id = $1 and ar.status = 'pending'
        order by ar.id
        limit 50
        "#,
    )
    .bind(org_id)
    .fetch_all(&state.db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| WorkQueueItem {
            kind: WorkQueueItemKind::ApprovalPending,
            title: format!("{} — {}", r.plot_number, r.customer_name),
            subtitle: format!("Requested by {}", r.requested_by_name),
            amount: Some(r.agreed_price),
            href: "/approvals".to_string(),
            due_date: None,
            days_overdue: None,
        })
        .collect())
}

#[derive(sqlx::FromRow)]
struct LeadFollowUpRow {
    id: Uuid,
    full_name: String,
    next_follow_up_at: NaiveDate,
}

async fn lead_follow_up_items(
    state: &AppState,
    org_id: Uuid,
    today: NaiveDate,
) -> Result<Vec<WorkQueueItem>, AppError> {
    let rows: Vec<LeadFollowUpRow> = sqlx::query_as(
        r#"
        select id, full_name, next_follow_up_at
        from customers
        where organization_id = $1 and stage != 'lost'
            and next_follow_up_at is not null and next_follow_up_at <= $2 + $3
        order by next_follow_up_at
        limit 50
        "#,
    )
    .bind(org_id)
    .bind(today)
    .bind(LOOKAHEAD_DAYS as i32)
    .fetch_all(&state.db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| {
            let days_overdue = (today - r.next_follow_up_at).num_days();
            WorkQueueItem {
                kind: WorkQueueItemKind::LeadFollowUp,
                title: r.full_name,
                subtitle: if days_overdue > 0 {
                    format!("Follow-up overdue by {days_overdue} day(s)")
                } else if days_overdue == 0 {
                    "Follow-up due today".to_string()
                } else {
                    format!("Follow-up due in {} day(s)", -days_overdue)
                },
                amount: None,
                href: format!("/customers/{}", r.id),
                due_date: Some(r.next_follow_up_at),
                days_overdue: (days_overdue > 0).then_some(days_overdue as i32),
            }
        })
        .collect())
}

#[derive(sqlx::FromRow)]
struct TenantPendingRow {
    id: Uuid,
    name: String,
    created_at: chrono::DateTime<Utc>,
}

async fn tenant_pending_items(state: &AppState) -> Result<Vec<WorkQueueItem>, AppError> {
    let rows: Vec<TenantPendingRow> = sqlx::query_as(
        "select id, name, created_at from organizations where status = 'pending_approval' order by created_at",
    )
    .fetch_all(&state.db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| WorkQueueItem {
            kind: WorkQueueItemKind::TenantPendingApproval,
            title: r.name,
            subtitle: format!("Applied {}", r.created_at.format("%b %d, %Y")),
            amount: None,
            href: format!("/platform/{}", r.id),
            due_date: None,
            days_overdue: None,
        })
        .collect())
}
