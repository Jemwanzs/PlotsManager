use axum::extract::Path;
use axum::routing::post;
use axum::{extract::State, routing::get, Json, Router};
use chrono::{DateTime, NaiveDate, Utc};
use domain::{
    LoanAccountDetail, LoanAccountStatus, Payment, PlotLoanAccount, RecordPaymentInput,
    PERM_PAYMENTS_RECORD,
};
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::error::AppError;
use crate::extractors::AuthUser;
use crate::pg_enum::{from_pg, to_pg};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/loan-accounts/:id", get(get_loan_account))
        .route("/api/v1/loan-accounts/:id/payments", post(record_payment))
}

#[derive(sqlx::FromRow)]
struct LoanAccountRow {
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
    // joined context
    plot_id: Uuid,
    plot_number: String,
    project_id: Uuid,
    project_name: String,
    customer_id: Uuid,
    customer_name: String,
}

const LOAN_ACCOUNT_DETAIL_QUERY: &str = r#"
    select pla.id, pla.account_number, pla.sale_id, pla.principal, pla.interest_rate,
        pla.deposit_required, pla.deposit_paid, pla.instalment_amount, pla.repayment_frequency_days,
        pla.start_date, pla.status, pla.amount_paid, pla.outstanding_balance, pla.days_in_arrears,
        pl.id as plot_id, pl.plot_number, pr.id as project_id, pr.name as project_name,
        c.id as customer_id, c.full_name as customer_name
    from plot_loan_accounts pla
    join plot_sales ps on ps.id = pla.sale_id
    join plots pl on pl.id = ps.plot_id
    join projects pr on pr.id = pl.project_id
    join customers c on c.id = ps.customer_id
    where pla.id = $1 and ps.organization_id = $2
"#;

#[derive(sqlx::FromRow)]
struct PaymentRow {
    id: Uuid,
    loan_account_id: Uuid,
    amount: Decimal,
    payment_date: NaiveDate,
    method: String,
    external_reference: Option<String>,
    status: String,
    captured_by: Uuid,
    verified_by: Option<Uuid>,
    created_at: DateTime<Utc>,
}

impl PaymentRow {
    fn into_domain(self) -> Result<Payment, AppError> {
        Ok(Payment {
            id: self.id,
            loan_account_id: self.loan_account_id,
            amount: self.amount,
            payment_date: self.payment_date,
            method: self.method,
            external_reference: self.external_reference,
            status: from_pg("payments.status", &self.status)?,
            captured_by: self.captured_by,
            verified_by: self.verified_by,
            created_at: self.created_at,
        })
    }
}

async fn get_loan_account(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<LoanAccountDetail>, AppError> {
    let row: Option<LoanAccountRow> = sqlx::query_as(LOAN_ACCOUNT_DETAIL_QUERY)
        .bind(id)
        .bind(auth.organization_id)
        .fetch_optional(&state.db)
        .await?;
    let row = row.ok_or(AppError::NotFound)?;

    let payment_rows: Vec<PaymentRow> = sqlx::query_as(
        "select id, loan_account_id, amount, payment_date, method, external_reference, status, captured_by, verified_by, created_at
         from payments where loan_account_id = $1 order by payment_date desc, created_at desc",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    let payments = payment_rows
        .into_iter()
        .map(PaymentRow::into_domain)
        .collect::<Result<Vec<_>, AppError>>()?;

    let status: LoanAccountStatus = from_pg("plot_loan_accounts.status", &row.status)?;
    let (label, color) = domain::loan_status_meta(status);

    Ok(Json(LoanAccountDetail {
        account: PlotLoanAccount {
            id: row.id,
            account_number: row.account_number,
            sale_id: row.sale_id,
            principal: row.principal,
            interest_rate: row.interest_rate,
            deposit_required: row.deposit_required,
            deposit_paid: row.deposit_paid,
            instalment_amount: row.instalment_amount,
            repayment_frequency_days: row.repayment_frequency_days,
            start_date: row.start_date,
            status,
            amount_paid: row.amount_paid,
            outstanding_balance: row.outstanding_balance,
            days_in_arrears: row.days_in_arrears,
        },
        plot_id: row.plot_id,
        plot_number: row.plot_number,
        project_id: row.project_id,
        project_name: row.project_name,
        customer_id: row.customer_id,
        customer_name: row.customer_name,
        status_label: label.to_string(),
        status_color: color.to_string(),
        payments,
    }))
}

/// Records a payment and updates the account's running balance/status in
/// one transaction — mirrors `frontend::api::mock::MockApi::record_payment`.
/// Posted immediately; the Captured -> Verified -> Posted approval gate
/// from docs/08/09 needs real roles first (tracked in
/// docs/14-development-roadmap.md), not half-built here.
async fn record_payment(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<RecordPaymentInput>,
) -> Result<Json<Payment>, AppError> {
    auth.require_permission(PERM_PAYMENTS_RECORD)?;

    if input.amount <= Decimal::ZERO {
        return Err(AppError::bad_request("Enter an amount greater than zero."));
    }
    if input.loan_account_id != id {
        return Err(AppError::bad_request("Loan account id mismatch."));
    }

    let mut tx = state.db.begin().await?;

    let account: Option<(Decimal, Decimal, Decimal)> = sqlx::query_as(
        r#"select pla.principal, pla.amount_paid, pla.outstanding_balance
           from plot_loan_accounts pla
           join plot_sales ps on ps.id = pla.sale_id
           where pla.id = $1 and ps.organization_id = $2
           for update of pla"#,
    )
    .bind(id)
    .bind(auth.organization_id)
    .fetch_optional(&mut *tx)
    .await?;
    let (_, amount_paid, outstanding_balance) = account.ok_or(AppError::NotFound)?;

    let new_amount_paid = amount_paid + input.amount;
    let new_outstanding = (outstanding_balance - input.amount).max(Decimal::ZERO);
    let new_status = if new_outstanding <= Decimal::ZERO {
        LoanAccountStatus::FullyPaid
    } else {
        LoanAccountStatus::ActivePartiallyPaid
    };

    let payment_row: PaymentRow = sqlx::query_as(
        r#"
        insert into payments (loan_account_id, amount, payment_date, method, status, captured_by, verified_by)
        values ($1, $2, $3, $4, 'posted', $5, $5)
        returning id, loan_account_id, amount, payment_date, method, external_reference, status, captured_by, verified_by, created_at
        "#,
    )
    .bind(id)
    .bind(input.amount)
    .bind(input.payment_date)
    .bind(&input.method)
    .bind(auth.user_id)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query("update plot_loan_accounts set amount_paid = $1, outstanding_balance = $2, status = $3 where id = $4")
        .bind(new_amount_paid)
        .bind(new_outstanding)
        .bind(to_pg(&new_status))
        .bind(id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    Ok(Json(payment_row.into_domain()?))
}
