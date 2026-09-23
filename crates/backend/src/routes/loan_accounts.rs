use axum::extract::{Path, Query};
use axum::routing::post;
use axum::{extract::State, routing::get, Json, Router};
use chrono::{DateTime, NaiveDate, Utc};
use domain::{
    ChargeType, LoanAccountDetail, LoanAccountStatus, LoanLedgerEntry, LoanStatement, Payment,
    PaymentAllocationPreview, PlotLoanAccount, PostChargeInput, RecordPaymentInput,
    PERM_FINANCE_POST_CHARGES, PERM_PAYMENTS_RECORD,
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
        .route("/api/v1/loan-accounts/:id/charges", post(post_charge))
        .route(
            "/api/v1/loan-accounts/:id/allocation-preview",
            get(preview_allocation),
        )
}

/// Penalty -> interest -> principal, the spec's default waterfall
/// order (a literal constant here rather than a config table until a
/// real "make this configurable" phase exists to attach a UI to it).
/// Shared by `record_payment` (which actually posts the result) and
/// `preview_allocation` (which only shows what it *would* be) so the
/// two can never drift apart.
fn allocate_waterfall(
    amount: Decimal,
    interest_outstanding: Decimal,
    penalty_outstanding: Decimal,
) -> (Decimal, Decimal, Decimal) {
    let mut remaining = amount;
    let penalty_paid = remaining.min(penalty_outstanding.max(Decimal::ZERO));
    remaining -= penalty_paid;
    let interest_paid = remaining.min(interest_outstanding.max(Decimal::ZERO));
    remaining -= interest_paid;
    let principal_paid = remaining;
    (penalty_paid, interest_paid, principal_paid)
}

async fn outstanding_components<'e, E: sqlx::PgExecutor<'e>>(
    db: E,
    loan_account_id: Uuid,
) -> Result<(Decimal, Decimal), AppError> {
    let (interest_outstanding, penalty_outstanding): (Decimal, Decimal) = sqlx::query_as(
        r#"select coalesce(sum(interest_delta), 0), coalesce(sum(penalty_delta), 0)
           from loan_ledger_entries where loan_account_id = $1"#,
    )
    .bind(loan_account_id)
    .fetch_one(db)
    .await?;
    Ok((interest_outstanding, penalty_outstanding))
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
    next_instalment_due_date: Option<NaiveDate>,
    next_instalment_amount: Option<Decimal>,
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
            next_instalment_due_date: row.next_instalment_due_date,
            next_instalment_amount: row.next_instalment_amount,
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

    let (interest_outstanding, penalty_outstanding) = outstanding_components(&mut *tx, id).await?;
    let (penalty_paid, interest_paid, principal_paid) =
        allocate_waterfall(input.amount, interest_outstanding, penalty_outstanding);

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
            next_instalment_due_date: row.next_instalment_due_date,
            next_instalment_amount: row.next_instalment_amount,
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

#[derive(serde::Deserialize)]
struct AllocationPreviewQuery {
    amount: Decimal,
}

/// What a payment of this size *would* clear, without posting
/// anything — same waterfall `record_payment` actually applies
/// (`allocate_waterfall`, shared by both), so what a user previews
/// before clicking "Record Payment" is guaranteed to match what
/// actually gets posted.
async fn preview_allocation(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Query(params): Query<AllocationPreviewQuery>,
) -> Result<Json<PaymentAllocationPreview>, AppError> {
    if params.amount <= Decimal::ZERO {
        return Err(AppError::bad_request("Enter an amount greater than zero."));
    }

    let outstanding_balance: Option<Decimal> = sqlx::query_scalar(
        r#"select pla.outstanding_balance from plot_loan_accounts pla
           join plot_sales ps on ps.id = pla.sale_id
           where pla.id = $1 and ps.organization_id = $2"#,
    )
    .bind(id)
    .bind(auth.organization_id)
    .fetch_optional(&state.db)
    .await?;
    let outstanding_balance = outstanding_balance.ok_or(AppError::NotFound)?;

    let (interest_outstanding, penalty_outstanding) = outstanding_components(&state.db, id).await?;
    let (penalty_paid, interest_paid, principal_paid) =
        allocate_waterfall(params.amount, interest_outstanding, penalty_outstanding);
    let new_balance = (outstanding_balance - params.amount).max(Decimal::ZERO);

    Ok(Json(PaymentAllocationPreview {
        amount: params.amount,
        penalty_paid,
        interest_paid,
        principal_paid,
        new_balance,
    }))
}

/// Posts a manual interest or penalty charge — the mechanic that
/// makes `loan_ledger_entries`' `charge_interest`/`charge_penalty`
/// entry types (and therefore the allocation waterfall's penalty/
/// interest-first behaviour) actually reachable; nothing else in this
/// app posts one. Increases the account's `outstanding_balance` by
/// the charge amount and, if the account had already reached
/// `FullyPaid` (a late penalty charged after the principal was
/// cleared), reopens it to `ActivePartiallyPaid` — a nonzero balance
/// can't coexist with a "fully paid" status. Does **not** touch the
/// plot's own status: a charge is a financial matter after the sale,
/// not a reversal of the sale itself.
async fn post_charge(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<PostChargeInput>,
) -> Result<Json<LoanLedgerEntry>, AppError> {
    auth.require_permission(PERM_FINANCE_POST_CHARGES)?;

    if input.loan_account_id != id {
        return Err(AppError::bad_request("Loan account id mismatch."));
    }
    if input.amount <= Decimal::ZERO {
        return Err(AppError::bad_request("Enter an amount greater than zero."));
    }
    let reason = input.reason.trim();
    if reason.is_empty() {
        return Err(AppError::bad_request("Enter a reason for this charge."));
    }

    let mut tx = state.db.begin().await?;

    let outstanding_balance: Option<Decimal> = sqlx::query_scalar(
        r#"select pla.outstanding_balance from plot_loan_accounts pla
           join plot_sales ps on ps.id = pla.sale_id
           where pla.id = $1 and ps.organization_id = $2
           for update of pla"#,
    )
    .bind(id)
    .bind(auth.organization_id)
    .fetch_optional(&mut *tx)
    .await?;
    let outstanding_balance = outstanding_balance.ok_or(AppError::NotFound)?;
    let new_balance = outstanding_balance + input.amount;

    let (entry_type, interest_delta, penalty_delta) = match input.charge_type {
        ChargeType::Interest => ("charge_interest", input.amount, Decimal::ZERO),
        ChargeType::Penalty => ("charge_penalty", Decimal::ZERO, input.amount),
    };

    #[derive(sqlx::FromRow)]
    struct NewChargeRow {
        id: Uuid,
        loan_account_id: Uuid,
        entry_type: String,
        entry_date: NaiveDate,
        gross_amount: Decimal,
        principal_delta: Decimal,
        interest_delta: Decimal,
        penalty_delta: Decimal,
        balance_after: Decimal,
        notes: Option<String>,
        created_at: DateTime<Utc>,
    }

    let entry_row: NewChargeRow = sqlx::query_as(
        r#"
        insert into loan_ledger_entries
            (loan_account_id, organization_id, entry_type, entry_date, gross_amount,
             principal_delta, interest_delta, penalty_delta, balance_after, notes, created_by)
        values ($1, $2, $3, $4, $5, 0, $6, $7, $8, $9, $10)
        returning id, loan_account_id, entry_type, entry_date, gross_amount, principal_delta,
            interest_delta, penalty_delta, balance_after, notes, created_at
        "#,
    )
    .bind(id)
    .bind(auth.organization_id)
    .bind(entry_type)
    .bind(input.charge_date)
    .bind(input.amount)
    .bind(interest_delta)
    .bind(penalty_delta)
    .bind(new_balance)
    .bind(reason)
    .bind(auth.user_id)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        r#"update plot_loan_accounts set outstanding_balance = $1,
               status = case when status = 'fully_paid' then 'active_partially_paid' else status end
           where id = $2"#,
    )
    .bind(new_balance)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let _ = sqlx::query(
        r#"insert into audit_log (organization_id, actor_id, entity_type, entity_id, action, after_state)
           values ($1, $2, 'loan_ledger_entry', $3, 'charge_posted', $4)"#,
    )
    .bind(auth.organization_id)
    .bind(auth.user_id)
    .bind(entry_row.id)
    .bind(serde_json::json!({
        "charge_type": input.charge_type,
        "amount": input.amount,
        "reason": reason,
    }))
    .execute(&state.db)
    .await;

    let created_by_name: String = sqlx::query_scalar("select full_name from users where id = $1")
        .bind(auth.user_id)
        .fetch_one(&state.db)
        .await?;

    Ok(Json(LoanLedgerEntry {
        id: entry_row.id,
        loan_account_id: entry_row.loan_account_id,
        entry_type: from_pg("loan_ledger_entries.entry_type", &entry_row.entry_type)?,
        entry_date: entry_row.entry_date,
        gross_amount: entry_row.gross_amount,
        principal_delta: entry_row.principal_delta,
        interest_delta: entry_row.interest_delta,
        penalty_delta: entry_row.penalty_delta,
        balance_after: entry_row.balance_after,
        method: None,
        external_reference: None,
        notes: entry_row.notes,
        created_by_name,
        created_at: entry_row.created_at,
    }))
}
