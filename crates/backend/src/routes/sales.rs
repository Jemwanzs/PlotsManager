use axum::{extract::State, routing::post, Json, Router};
use chrono::NaiveTime;
use domain::{
    BulkImportResult, BulkImportRowError, BulkSaleRow, CreateSaleInput, LoanAccountStatus,
    PaymentMode, PlotSale, PlotStatus,
};
use rust_decimal::Decimal;
use uuid::Uuid;

use super::approvals::gate_price;
use crate::error::AppError;
use crate::extractors::AuthUser;
use crate::pg_enum::to_pg;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/sales", post(create_sale))
        .route("/api/v1/sales/bulk", post(bulk_create_sales))
}

/// Reserves a plot for a customer — the first step of the sales workflow
/// (docs/07). Mirrors `frontend::api::mock::MockApi::create_sale` exactly
/// (same 10% deposit / 12-instalment Plot Loan Account default for Lipa
/// Pole Pole) so the UI behaves identically against either.
async fn create_sale(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<CreateSaleInput>,
) -> Result<Json<PlotSale>, AppError> {
    // Below the plot's minimum_price? gate_price records/consumes an
    // approval before we ever open the sale transaction — see its docs
    // on why that has to happen against the pool, not this tx.
    let approval_id = gate_price(
        &state.db,
        auth.organization_id,
        auth.user_id,
        input.plot_id,
        input.customer_id,
        Some(auth.user_id),
        input.payment_mode,
        input.agreed_price,
        None,
    )
    .await?;

    let mut tx = state.db.begin().await?;

    let sale = execute_sale(
        &mut tx,
        auth.organization_id,
        ExecuteSaleParams {
            plot_id: input.plot_id,
            customer_id: input.customer_id,
            agent_id: auth.user_id,
            payment_mode: input.payment_mode,
            agreed_price: input.agreed_price,
        },
    )
    .await?;

    if let Some(approval_id) = approval_id {
        sqlx::query("update approval_requests set resulting_sale_id = $1 where id = $2")
            .bind(sale.id)
            .bind(approval_id)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;

    Ok(Json(sale))
}

pub(crate) struct ExecuteSaleParams {
    pub plot_id: Uuid,
    pub customer_id: Uuid,
    pub agent_id: Uuid,
    pub payment_mode: PaymentMode,
    pub agreed_price: Decimal,
}

/// The actual "commit to a sale" transaction — plot/customer validation,
/// the `plot_sales` insert, the Plot Loan Account for non-cash modes, and
/// the plot status flip, all against the transaction the caller already
/// owns. Shared by `create_sale` above (reserving a plot directly) and
/// `routes/quotations.rs`'s `accept_quotation` (converting a customer-
/// accepted quotation into the same kind of real sale) — one place that
/// knows what "becoming a sale" means, so the two paths can't drift.
pub(crate) async fn execute_sale(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    organization_id: Uuid,
    params: ExecuteSaleParams,
) -> Result<PlotSale, AppError> {
    if params.agreed_price <= Decimal::ZERO {
        return Err(AppError::bad_request(
            "Enter an agreed price greater than zero.",
        ));
    }

    let plot_ok: bool = sqlx::query_scalar(
        r#"select exists(
            select 1 from plots pl join projects p on p.id = pl.project_id
            where pl.id = $1 and p.organization_id = $2
        )"#,
    )
    .bind(params.plot_id)
    .bind(organization_id)
    .fetch_one(&mut **tx)
    .await?;
    if !plot_ok {
        return Err(AppError::NotFound);
    }

    let customer_ok: bool = sqlx::query_scalar(
        "select exists(select 1 from customers where id = $1 and organization_id = $2)",
    )
    .bind(params.customer_id)
    .bind(organization_id)
    .fetch_one(&mut **tx)
    .await?;
    if !customer_ok {
        return Err(AppError::bad_request("Choose a valid customer."));
    }

    let sale_id: Uuid = sqlx::query_scalar(
        r#"
        insert into plot_sales (plot_id, customer_id, organization_id, agent_id, payment_mode, agreed_price)
        values ($1, $2, $3, $4, $5, $6)
        returning id
        "#,
    )
    .bind(params.plot_id)
    .bind(params.customer_id)
    .bind(organization_id)
    .bind(params.agent_id)
    .bind(to_pg(&params.payment_mode))
    .bind(params.agreed_price)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db_err)
            if db_err.constraint() == Some("plot_sales_one_active_per_plot") =>
        {
            AppError::conflict("This plot already has an active sale.")
        }
        _ => AppError::from(e),
    })?;

    if params.payment_mode != PaymentMode::FullCash {
        let account_number: String = sqlx::query_scalar(
            "select 'PLA-' || lpad(nextval('plot_loan_account_number_seq')::text, 4, '0')",
        )
        .fetch_one(&mut **tx)
        .await?;

        let deposit_required = (params.agreed_price * Decimal::new(10, 2)).round();
        let financed = params.agreed_price - deposit_required;
        let instalment_amount = (financed / Decimal::from(12)).round();
        let interest_rate = match params.payment_mode {
            PaymentMode::LipaPolePoleInterestBearing => Some(Decimal::from(14)),
            _ => None,
        };

        sqlx::query(
            r#"
            insert into plot_loan_accounts
                (account_number, sale_id, principal, interest_rate, deposit_required, deposit_paid,
                 instalment_amount, repayment_frequency_days, start_date, status, amount_paid, outstanding_balance)
            values ($1, $2, $3, $4, $5, 0, $6, 30, current_date, 'approved_awaiting_deposit', 0, $3)
            "#,
        )
        .bind(&account_number)
        .bind(sale_id)
        .bind(params.agreed_price)
        .bind(interest_rate)
        .bind(deposit_required)
        .bind(instalment_amount)
        .execute(&mut **tx)
        .await?;
    }

    let new_status = match params.payment_mode {
        PaymentMode::FullCash => PlotStatus::Reserved,
        PaymentMode::LipaPolePoleInterestFree | PaymentMode::LipaPolePoleInterestBearing => {
            PlotStatus::Booked
        }
    };
    sqlx::query("update plots set status = $1, assigned_customer_id = $2 where id = $3")
        .bind(to_pg(&new_status))
        .bind(params.customer_id)
        .bind(params.plot_id)
        .execute(&mut **tx)
        .await?;

    let sale: PlotSaleRow = sqlx::query_as(
        r#"select id, plot_id, customer_id, organization_id, agent_id, payment_mode, agreed_price, created_at
           from plot_sales where id = $1"#,
    )
    .bind(sale_id)
    .fetch_one(&mut **tx)
    .await?;

    sale.into_domain()
}

/// Best-effort bulk import of *historical* sales (tenant onboarding —
/// see `domain::BulkSaleRow`'s module docs for why this is a separate
/// path from `execute_sale`, which is for a fresh reservation made
/// today). A CSV upload parsed client-side to `BulkSaleRow` rows and
/// posted here.
async fn bulk_create_sales(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(inputs): Json<Vec<BulkSaleRow>>,
) -> Result<Json<BulkImportResult>, AppError> {
    let mut created = 0u32;
    let mut errors = Vec::new();
    for (idx, input) in inputs.iter().enumerate() {
        match insert_bulk_sale(&state, auth.organization_id, input).await {
            Ok(_) => created += 1,
            Err(e) => errors.push(BulkImportRowError {
                row: idx as u32 + 1,
                message: e.client_message(),
            }),
        }
    }
    Ok(Json(BulkImportResult { created, errors }))
}

/// Unlike `execute_sale`, this owns its own transaction (one per row,
/// not one for the whole batch — see `domain::BulkImportResult`'s
/// "best-effort, not all-or-nothing" module docs) and computes a Plot
/// Loan Account's starting state from `amount_paid` instead of
/// assuming zero: `status`, `deposit_paid` and `outstanding_balance`
/// are all derived from how much has actually been repaid, matching
/// this codebase's "derive, don't store" preference for anything that
/// can be worked out from a fact already on hand.
async fn insert_bulk_sale(
    state: &AppState,
    organization_id: Uuid,
    input: &BulkSaleRow,
) -> Result<(), AppError> {
    if input.agreed_price <= Decimal::ZERO {
        return Err(AppError::bad_request(
            "Enter an agreed price greater than zero.",
        ));
    }
    let amount_paid = input.amount_paid.max(Decimal::ZERO).min(input.agreed_price);

    let plot_id: Option<Uuid> = sqlx::query_scalar(
        r#"select pl.id from plots pl
           join projects p on p.id = pl.project_id
           where p.organization_id = $1 and p.code = $2 and pl.plot_number = $3"#,
    )
    .bind(organization_id)
    .bind(&input.project_code)
    .bind(&input.plot_number)
    .fetch_optional(&state.db)
    .await?;
    let plot_id = plot_id.ok_or_else(|| {
        AppError::bad_request(format!(
            "No plot \"{}\" found in project \"{}\".",
            input.plot_number, input.project_code
        ))
    })?;

    let lookup = input.customer_lookup.trim();
    if lookup.is_empty() {
        return Err(AppError::bad_request(
            "Provide a customer ID number, phone, or email to match an existing customer.",
        ));
    }
    let customer_id: Option<Uuid> = sqlx::query_scalar(
        r#"select id from customers
           where organization_id = $1 and (id_number = $2 or phone = $2 or email = $2)
           limit 1"#,
    )
    .bind(organization_id)
    .bind(lookup)
    .fetch_optional(&state.db)
    .await?;
    let customer_id = customer_id.ok_or_else(|| {
        AppError::bad_request(format!(
            "No existing customer matches \"{lookup}\" — import customers first."
        ))
    })?;

    // Anchored at noon UTC, not midnight — a plain midnight timestamp
    // sits right on a day boundary and can display as the day before
    // in a timezone west of UTC; noon has slack on both sides.
    let sale_at = input.sale_date.and_time(NaiveTime::from_hms_opt(12, 0, 0).unwrap()).and_utc();

    let mut tx = state.db.begin().await?;

    let sale_id: Uuid = sqlx::query_scalar(
        r#"
        insert into plot_sales (plot_id, customer_id, organization_id, payment_mode, agreed_price, created_at)
        values ($1, $2, $3, $4, $5, $6)
        returning id
        "#,
    )
    .bind(plot_id)
    .bind(customer_id)
    .bind(organization_id)
    .bind(to_pg(&input.payment_mode))
    .bind(input.agreed_price)
    .bind(sale_at)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db_err)
            if db_err.constraint() == Some("plot_sales_one_active_per_plot") =>
        {
            AppError::conflict("This plot already has an active sale.")
        }
        _ => AppError::from(e),
    })?;

    // A historical import records an outcome, not the start of
    // today's workflow — `execute_sale` puts a brand-new cash sale in
    // 'reserved' because nothing has been finalized yet; an imported
    // one already has been.
    let fully_paid = input.payment_mode == PaymentMode::FullCash || amount_paid >= input.agreed_price;
    let new_status = if fully_paid {
        PlotStatus::Sold
    } else {
        PlotStatus::Booked
    };

    if input.payment_mode != PaymentMode::FullCash {
        let account_number: String = sqlx::query_scalar(
            "select 'PLA-' || lpad(nextval('plot_loan_account_number_seq')::text, 4, '0')",
        )
        .fetch_one(&mut *tx)
        .await?;

        let deposit_required = (input.agreed_price * Decimal::new(10, 2)).round();
        let financed = input.agreed_price - deposit_required;
        let instalment_amount = (financed / Decimal::from(12)).round();
        let interest_rate = match input.payment_mode {
            PaymentMode::LipaPolePoleInterestBearing => Some(Decimal::from(14)),
            _ => None,
        };
        let deposit_paid = amount_paid.min(deposit_required);
        let outstanding_balance = input.agreed_price - amount_paid;
        let status = if fully_paid {
            LoanAccountStatus::FullyPaid
        } else if amount_paid > Decimal::ZERO {
            LoanAccountStatus::ActivePartiallyPaid
        } else {
            LoanAccountStatus::ApprovedAwaitingDeposit
        };

        sqlx::query(
            r#"
            insert into plot_loan_accounts
                (account_number, sale_id, principal, interest_rate, deposit_required, deposit_paid,
                 instalment_amount, repayment_frequency_days, start_date, status, amount_paid, outstanding_balance)
            values ($1, $2, $3, $4, $5, $6, $7, 30, $8, $9, $10, $11)
            "#,
        )
        .bind(&account_number)
        .bind(sale_id)
        .bind(input.agreed_price)
        .bind(interest_rate)
        .bind(deposit_required)
        .bind(deposit_paid)
        .bind(instalment_amount)
        .bind(input.sale_date)
        .bind(to_pg(&status))
        .bind(amount_paid)
        .bind(outstanding_balance)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query("update plots set status = $1, assigned_customer_id = $2 where id = $3")
        .bind(to_pg(&new_status))
        .bind(customer_id)
        .bind(plot_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(())
}

#[derive(sqlx::FromRow)]
struct PlotSaleRow {
    id: Uuid,
    plot_id: Uuid,
    customer_id: Uuid,
    organization_id: Uuid,
    agent_id: Option<Uuid>,
    payment_mode: String,
    agreed_price: Decimal,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl PlotSaleRow {
    fn into_domain(self) -> Result<PlotSale, AppError> {
        Ok(PlotSale {
            id: self.id,
            plot_id: self.plot_id,
            customer_id: self.customer_id,
            organization_id: self.organization_id,
            agent_id: self.agent_id,
            payment_mode: crate::pg_enum::from_pg("plot_sales.payment_mode", &self.payment_mode)?,
            agreed_price: self.agreed_price,
            created_at: self.created_at,
        })
    }
}
