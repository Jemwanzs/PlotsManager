use axum::extract::Path;
use axum::routing::post;
use axum::{extract::State, routing::get, Json, Router};
use chrono::{DateTime, NaiveDate, Utc};
use domain::{
    LoanAccountDetail, LoanAccountStatus, LoanLedgerEntry, LoanStatement, Payment,
    PlotLoanAccount, RecordPaymentInput, PERM_PAYMENTS_RECORD,
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
        .route("/api/v1/loan-accounts/:id/statement", get(get_loan_statement))
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

    // Allocation waterfall: penalty -> interest -> principal (the
    // order the enhancement spec calls for as the default, kept as a
    // literal constant here rather than a config table until a real
    // "make this configurable" phase exists to attach a UI to it).
    // Outstanding interest/penalty are the running sums of every
    // charge and waiver posted so far for this account — currently
    // always zero, since nothing anywhere posts an interest or
    // penalty charge yet, so every payment allocates entirely to
    // principal until that phase ships. Written generically now so it
    // needs no changes once charges exist.
    let (interest_outstanding, penalty_outstanding): (Decimal, Decimal) = sqlx::query_as(
        r#"select coalesce(sum(interest_delta), 0), coalesce(sum(penalty_delta), 0)
           from loan_ledger_entries where loan_account_id = $1"#,
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;

    let mut remaining = input.amount;
    let penalty_paid = remaining.min(penalty_outstanding.max(Decimal::ZERO));
    remaining -= penalty_paid;
    let interest_paid = remaining.min(interest_outstanding.max(Decimal::ZERO));
    remaining -= interest_paid;
    let principal_paid = remaining;

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

    sqlx::query(
        r#"
        insert into loan_ledger_entries
            (loan_account_id, organization_id, entry_type, entry_date, gross_amount,
             principal_delta, interest_delta, penalty_delta, balance_after,
             method, external_reference, reference_payment_id, created_by)
        values ($1, $2, 'payment', $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        "#,
    )
    .bind(id)
    .bind(auth.organization_id)
    .bind(input.payment_date)
    .bind(input.amount)
    .bind(-principal_paid)
    .bind(-interest_paid)
    .bind(-penalty_paid)
    .bind(new_outstanding)
    .bind(&input.method)
    .bind(&payment_row.external_reference)
    .bind(payment_row.id)
    .bind(auth.user_id)
    .execute(&mut *tx)
    .await?;

    // The plot itself never advanced past `Booked` once this reached
    // `FullyPaid` — nothing else in the app ever touched `plots.status`
    // after the sale was first recorded, so a fully-repaid Lipa Pole
    // Pole plot stayed looking "Booked" forever on every screen that
    // reads plot status (list, map, customer, reports). Only advances
    // forward: a plot already moved on to a later stage (transfer in
    // progress/transferred/blocked/disputed/cancelled — none reachable
    // today, but this guards against ever clobbering one) is left
    // alone rather than pulled back to `Sold`.
    if new_status == LoanAccountStatus::FullyPaid {
        sqlx::query(
            r#"
            update plots set status = 'sold'
            where id = (select pl.id from plots pl
                        join plot_sales ps on ps.plot_id = pl.id
                        join plot_loan_accounts pla on pla.sale_id = ps.id
                        where pla.id = $1)
              and status in ('booked', 'reserved', 'selected', 'temporarily_held', 'under_approval')
            "#,
        )
        .bind(id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(Json(payment_row.into_domain()?))
}

#[derive(sqlx::FromRow)]
struct LedgerEntryRow {
    id: Uuid,
    loan_account_id: Uuid,
    entry_type: String,
    entry_date: NaiveDate,
    gross_amount: Decimal,
    principal_delta: Decimal,
    interest_delta: Decimal,
    penalty_delta: Decimal,
    balance_after: Decimal,
    method: Option<String>,
    external_reference: Option<String>,
    notes: Option<String>,
    created_by_name: String,
    created_at: DateTime<Utc>,
}

impl LedgerEntryRow {
    fn into_domain(self) -> Result<LoanLedgerEntry, AppError> {
        Ok(LoanLedgerEntry {
            id: self.id,
            loan_account_id: self.loan_account_id,
            entry_type: from_pg("loan_ledger_entries.entry_type", &self.entry_type)?,
            entry_date: self.entry_date,
            gross_amount: self.gross_amount,
            principal_delta: self.principal_delta,
            interest_delta: self.interest_delta,
            penalty_delta: self.penalty_delta,
            balance_after: self.balance_after,
            method: self.method,
            external_reference: self.external_reference,
            notes: self.notes,
            created_by_name: self.created_by_name,
            created_at: self.created_at,
        })
    }
}

/// The full running statement for one receivable account — header
/// context plus every ledger entry in order, generated from the
/// actual transaction ledger (`loan_ledger_entries`), never
/// reconstructed from the account's current balance alone.
async fn get_loan_statement(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<LoanStatement>, AppError> {
    let row: Option<LoanAccountRow> = sqlx::query_as(LOAN_ACCOUNT_DETAIL_QUERY)
        .bind(id)
        .bind(auth.organization_id)
        .fetch_optional(&state.db)
        .await?;
    let row = row.ok_or(AppError::NotFound)?;

    let agreed_price: Decimal =
        sqlx::query_scalar("select agreed_price from plot_sales where id = $1")
            .bind(row.sale_id)
            .fetch_one(&state.db)
            .await?;

    let entry_rows: Vec<LedgerEntryRow> = sqlx::query_as(
        r#"
        select le.id, le.loan_account_id, le.entry_type, le.entry_date, le.gross_amount,
            le.principal_delta, le.interest_delta, le.penalty_delta, le.balance_after,
            le.method, le.external_reference, le.notes, u.full_name as created_by_name,
            le.created_at
        from loan_ledger_entries le
        join users u on u.id = le.created_by
        where le.loan_account_id = $1
        order by le.entry_date, le.created_at
        "#,
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    let entries = entry_rows
        .into_iter()
        .map(LedgerEntryRow::into_domain)
        .collect::<Result<Vec<_>, AppError>>()?;

    let status: LoanAccountStatus = from_pg("plot_loan_accounts.status", &row.status)?;
    let (label, color) = domain::loan_status_meta(status);

    Ok(Json(LoanStatement {
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
        plot_number: row.plot_number,
        project_name: row.project_name,
        customer_name: row.customer_name,
        agreed_price,
        status_label: label.to_string(),
        status_color: color.to_string(),
        entries,
    }))
}
