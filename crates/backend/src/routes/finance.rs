//! Finance module — the organization-wide view across every Lipa Pole
//! Pole receivable, which previously only existed per-customer (drilling
//! into a sale's own loan account). Read-only for now: creating/editing
//! loan accounts still happens through the sales workflow
//! (`routes/sales.rs`, `routes/loan_accounts.rs`), not here.

use axum::{extract::State, routing::get, Json, Router};
use chrono::NaiveDate;
use domain::{FinanceReceivablesBreakdown, LoanAccountStatus, LoanAccountSummary, PlotLoanAccount};
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::error::AppError;
use crate::extractors::AuthUser;
use crate::pg_enum::from_pg;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/finance/loan-accounts", get(list_loan_accounts))
        .route("/api/v1/finance/receivables-breakdown", get(receivables_breakdown))
}

#[derive(sqlx::FromRow)]
struct LoanAccountSummaryRow {
    id: Uuid,
    account_number: String,
    sale_id: Uuid,
    principal: Decimal,
    interest_rate: Option<Decimal>,
    deposit_required: Decimal,
    deposit_paid: Decimal,
    instalment_amount: Decimal,
    repayment_frequency_days: i32,
    start_date: NaiveDate,
    status: String,
    amount_paid: Decimal,
    outstanding_balance: Decimal,
    days_in_arrears: i32,
    next_instalment_due_date: Option<NaiveDate>,
    next_instalment_amount: Option<Decimal>,
    plot_id: Uuid,
    plot_number: String,
    project_id: Uuid,
    project_name: String,
    customer_id: Uuid,
    customer_name: String,
}

async fn list_loan_accounts(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<LoanAccountSummary>>, AppError> {
    let rows: Vec<LoanAccountSummaryRow> = sqlx::query_as(
        r#"
        select pla.id, pla.account_number, pla.sale_id, pla.principal, pla.interest_rate,
            pla.deposit_required, pla.deposit_paid, pla.instalment_amount, pla.repayment_frequency_days,
            pla.start_date, pla.status, pla.amount_paid, pla.outstanding_balance,
            coalesce((current_date - lass.oldest_overdue_due_date), 0) as days_in_arrears,
            lass.next_instalment_due_date, lass.next_instalment_amount,
            pl.id as plot_id, pl.plot_number, pr.id as project_id, pr.name as project_name,
            c.id as customer_id, c.full_name as customer_name
        from plot_loan_accounts pla
        join plot_sales ps on ps.id = pla.sale_id
        join plots pl on pl.id = ps.plot_id
        join projects pr on pr.id = pl.project_id
        join customers c on c.id = ps.customer_id
        left join loan_account_schedule_summary lass on lass.loan_account_id = pla.id
        where ps.organization_id = $1
        order by pla.outstanding_balance desc, pla.start_date desc
        "#,
    )
    .bind(auth.organization_id)
    .fetch_all(&state.db)
    .await?;

    let summaries = rows
        .into_iter()
        .map(|r| {
            let status: LoanAccountStatus = from_pg("plot_loan_accounts.status", &r.status)?;
            let (label, color) = domain::loan_status_meta(status);
            Ok(LoanAccountSummary {
                account: PlotLoanAccount {
                    id: r.id,
                    account_number: r.account_number,
                    sale_id: r.sale_id,
                    principal: r.principal,
                    interest_rate: r.interest_rate,
                    deposit_required: r.deposit_required,
                    deposit_paid: r.deposit_paid,
                    instalment_amount: r.instalment_amount,
                    repayment_frequency_days: r.repayment_frequency_days,
                    start_date: r.start_date,
                    status,
                    amount_paid: r.amount_paid,
                    outstanding_balance: r.outstanding_balance,
                    days_in_arrears: r.days_in_arrears,
                    next_instalment_due_date: r.next_instalment_due_date,
                    next_instalment_amount: r.next_instalment_amount,
                },
                plot_id: r.plot_id,
                plot_number: r.plot_number,
                project_id: r.project_id,
                project_name: r.project_name,
                customer_id: r.customer_id,
                customer_name: r.customer_name,
                status_label: label.to_string(),
                status_color: color.to_string(),
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;

    Ok(Json(summaries))
}

#[derive(sqlx::FromRow)]
struct ReceivablesBreakdownRow {
    interest_outstanding: Decimal,
    penalty_outstanding: Decimal,
}

/// Sums every ledger entry's `interest_delta`/`penalty_delta` across the
/// whole org — the net outstanding for each component, since a payment
/// or waiver's delta is negative and a charge's is positive. Mirrors
/// `outstanding_components` in `routes/loan_accounts.rs`, which does the
/// same sum scoped to one account.
async fn receivables_breakdown(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<FinanceReceivablesBreakdown>, AppError> {
    let row: ReceivablesBreakdownRow = sqlx::query_as(
        r#"
        select
            coalesce(sum(le.interest_delta), 0) as interest_outstanding,
            coalesce(sum(le.penalty_delta), 0) as penalty_outstanding
        from loan_ledger_entries le
        join plot_loan_accounts pla on pla.id = le.loan_account_id
        join plot_sales ps on ps.id = pla.sale_id
        where ps.organization_id = $1
        "#,
    )
    .bind(auth.organization_id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(FinanceReceivablesBreakdown {
        interest_outstanding: row.interest_outstanding,
        penalty_outstanding: row.penalty_outstanding,
    }))
}
