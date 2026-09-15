use axum::{extract::State, routing::post, Json, Router};
use domain::{CreateSaleInput, PaymentMode, PlotSale, PlotStatus};
use rust_decimal::Decimal;
use uuid::Uuid;

use super::approvals::gate_price;
use crate::error::AppError;
use crate::extractors::AuthUser;
use crate::pg_enum::to_pg;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/v1/sales", post(create_sale))
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
