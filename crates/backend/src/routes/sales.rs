use axum::{extract::Path, extract::State, routing::post, Json, Router};
use chrono::NaiveTime;
use domain::{
    AdditionalSaleCustomer, BulkImportResult, BulkImportRowError, BulkSaleRow, CancelSaleInput,
    CreateSaleInput, LoanAccountStatus, PaymentMode, PlotSale, PlotStatus, RepossessSaleInput,
    PERM_PLOTS_TRANSACTIONS_BULK_IMPORT, PERM_PLOTS_TRANSACTIONS_CANCEL,
    PERM_PLOTS_TRANSACTIONS_CREATE,
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
        .route("/api/v1/sales/:id/cancel", post(cancel_sale))
        .route("/api/v1/sales/:id/repossess", post(repossess_sale))
}

/// The static repayment plan behind a Plot Loan Account — a deposit
/// (instalment 0) plus 12 equal instalments, `repayment_frequency_days`
/// apart starting at `start_date`. Matches exactly what
/// 0022_repayment_schedule.sql backfilled for every account that
/// predates this table being populated, so a new account and a
/// backfilled one are computed by the same views (`loan_account_
/// schedule_summary` etc.) with no special-casing either way. Interest
/// isn't amortized in here — it's layered on separately via manual
/// charges (`routes/loan_accounts.rs::post_charge`) — so every row's
/// `interest_due` stays 0.
async fn insert_repayment_schedule<'e, E: sqlx::PgExecutor<'e>>(
    db: E,
    loan_account_id: Uuid,
    deposit_required: Decimal,
    instalment_amount: Decimal,
    repayment_frequency_days: i32,
    start_date: chrono::NaiveDate,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        insert into repayment_schedule_entries
            (loan_account_id, instalment_number, due_date, principal_due, interest_due, fees_due, total_due, amount_paid, status)
        select $1, 0, $5, $2, 0, 0, $2, 0, 'upcoming'
        where $2 > 0
        union all
        select $1, gs.n, $5 + ($4 * gs.n) * interval '1 day', $3, 0, 0, $3, 0, 'upcoming'
        from generate_series(1, 12) as gs(n)
        where $3 > 0
        "#,
    )
    .bind(loan_account_id)
    .bind(deposit_required)
    .bind(instalment_amount)
    .bind(repayment_frequency_days)
    .bind(start_date)
    .execute(db)
    .await?;
    Ok(())
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
    auth.require_permission(PERM_PLOTS_TRANSACTIONS_CREATE)?;

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
            additional_plot_ids: input.additional_plot_ids,
            additional_customers: input.additional_customers,
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
    /// Other plots this same sale/loan also covers — see
    /// `database/migrations/0026_sale_plots_and_customers.sql`. Empty
    /// from `accept_quotation` (a quotation is for one plot; multi-plot
    /// sales only exist via the direct reserve flow for now).
    pub additional_plot_ids: Vec<Uuid>,
    /// Other buyers on this sale beyond `customer_id`. Empty from
    /// `accept_quotation`, same reason as above.
    pub additional_customers: Vec<AdditionalSaleCustomer>,
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

    // The partial unique indexes (`plot_sales_one_active_per_plot`/
    // `sale_plots_plot_uidx`, `database/migrations/0030_sale_lifecycle.sql`)
    // only stop a *second active* sale on the same plot — they don't
    // stop a brand new one on a plot that's `Cancelled`/`Blocked` from
    // a prior cancel/repossess, since neither of those states leaves
    // any *active* `plot_sales` row behind to collide with. That gap
    // needs its own check: those two states must go through
    // `routes/projects.rs::reallocate_plot` first, same as the
    // frontend's own `can_start_sale` gate already assumes.
    let plot_status: Option<String> = sqlx::query_scalar(
        r#"select pl.status from plots pl join projects p on p.id = pl.project_id
           where pl.id = $1 and p.organization_id = $2"#,
    )
    .bind(params.plot_id)
    .bind(organization_id)
    .fetch_optional(&mut **tx)
    .await?;
    match plot_status.as_deref() {
        None => return Err(AppError::NotFound),
        Some("cancelled") | Some("blocked") => {
            return Err(AppError::conflict(
                "This plot's previous sale was cancelled or repossessed — reallocate it before starting a new sale.",
            ));
        }
        _ => {}
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

    for &plot_id in &params.additional_plot_ids {
        if plot_id == params.plot_id {
            return Err(AppError::bad_request(
                "The same plot can't be listed as both the primary plot and an additional one.",
            ));
        }
        let ok: bool = sqlx::query_scalar(
            r#"select exists(
                select 1 from plots pl join projects p on p.id = pl.project_id
                where pl.id = $1 and p.organization_id = $2
            )"#,
        )
        .bind(plot_id)
        .bind(organization_id)
        .fetch_one(&mut **tx)
        .await?;
        if !ok {
            return Err(AppError::bad_request("One of the additional plots doesn't exist."));
        }
    }
    for extra in &params.additional_customers {
        if extra.customer_id == params.customer_id {
            return Err(AppError::bad_request(
                "The same customer can't be listed as both the primary buyer and an additional one.",
            ));
        }
        let ok: bool = sqlx::query_scalar(
            "select exists(select 1 from customers where id = $1 and organization_id = $2)",
        )
        .bind(extra.customer_id)
        .bind(organization_id)
        .fetch_one(&mut **tx)
        .await?;
        if !ok {
            return Err(AppError::bad_request("One of the additional buyers doesn't exist."));
        }
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

    sqlx::query("insert into sale_plots (sale_id, plot_id) values ($1, $2)")
        .bind(sale_id)
        .bind(params.plot_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| match &e {
            sqlx::Error::Database(db_err)
                if db_err.constraint() == Some("sale_plots_plot_uidx") =>
            {
                // Reachable even though the `plot_sales` insert above
                // just succeeded: that only enforces uniqueness on
                // `plot_sales.plot_id` (the *primary* plot column), so
                // a plot that was only ever an *additional* plot on
                // some other sale passes that check but still
                // collides here.
                AppError::conflict("This plot already has an active sale.")
            }
            _ => AppError::from(e),
        })?;
    for &plot_id in &params.additional_plot_ids {
        sqlx::query("insert into sale_plots (sale_id, plot_id) values ($1, $2)")
            .bind(sale_id)
            .bind(plot_id)
            .execute(&mut **tx)
            .await
            .map_err(|e| match &e {
                sqlx::Error::Database(db_err)
                    if db_err.constraint() == Some("sale_plots_plot_uidx") =>
                {
                    AppError::conflict("One of the additional plots already has an active sale.")
                }
                _ => AppError::from(e),
            })?;
    }

    sqlx::query("insert into sale_customers (sale_id, customer_id, role) values ($1, $2, 'primary')")
        .bind(sale_id)
        .bind(params.customer_id)
        .execute(&mut **tx)
        .await?;
    for extra in &params.additional_customers {
        sqlx::query("insert into sale_customers (sale_id, customer_id, role) values ($1, $2, $3)")
            .bind(sale_id)
            .bind(extra.customer_id)
            .bind(to_pg(&extra.role))
            .execute(&mut **tx)
            .await?;
    }

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

        let (loan_account_id, start_date): (Uuid, chrono::NaiveDate) = sqlx::query_as(
            r#"
            insert into plot_loan_accounts
                (account_number, sale_id, principal, interest_rate, deposit_required, deposit_paid,
                 instalment_amount, repayment_frequency_days, start_date, status, amount_paid, outstanding_balance)
            values ($1, $2, $3, $4, $5, 0, $6, 30, current_date, 'approved_awaiting_deposit', 0, $3)
            returning id, start_date
            "#,
        )
        .bind(&account_number)
        .bind(sale_id)
        .bind(params.agreed_price)
        .bind(interest_rate)
        .bind(deposit_required)
        .bind(instalment_amount)
        .fetch_one(&mut **tx)
        .await?;

        insert_repayment_schedule(&mut **tx, loan_account_id, deposit_required, instalment_amount, 30, start_date)
            .await?;
    }

    // A full-cash sale is paid in full at the moment it's recorded — no
    // loan account, no follow-up payment step exists for it anywhere in
    // this app (see `record_payment`'s own docs), so leaving it at
    // `Reserved` left every cash sale permanently stuck looking
    // unfinished. `Booked` for Lipa Pole Pole is correct as a starting
    // point precisely because there *is* a follow-up: `record_payment`
    // advances it to `Sold` once the loan account reaches `FullyPaid`
    // (see the status-sync call at the end of that handler).
    let new_status = match params.payment_mode {
        PaymentMode::FullCash => PlotStatus::Sold,
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
    for &plot_id in &params.additional_plot_ids {
        sqlx::query("update plots set status = $1, assigned_customer_id = $2 where id = $3")
            .bind(to_pg(&new_status))
            .bind(params.customer_id)
            .bind(plot_id)
            .execute(&mut **tx)
            .await?;
    }

    let sale: PlotSaleRow = sqlx::query_as(
        r#"select id, plot_id, customer_id, organization_id, agent_id, payment_mode, agreed_price, created_at
           from plot_sales where id = $1"#,
    )
    .bind(sale_id)
    .fetch_one(&mut **tx)
    .await?;

    sale.into_domain()
}

/// Administrative/mutual cancellation — the customer backs out, the
/// sale was a data-entry mistake, both sides agree to unwind it. No
/// loan account requirement (unlike `repossess_sale`): a full-cash
/// sale can be cancelled too. Every plot on the sale (primary and
/// additional) moves to `Cancelled` and is unassigned; a linked loan
/// account (if any) moves to `Cancelled` too, its `outstanding_balance`
/// left untouched — a historical record of what was owed, not
/// something this action forgives (`finance:reverse`/waivers are the
/// separate capability for that). See `database/migrations/
/// 0030_sale_lifecycle.sql` for why `plot_sales`/`sale_plots` can now
/// carry more than one row per plot over time.
async fn cancel_sale(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(sale_id): Path<Uuid>,
    Json(input): Json<CancelSaleInput>,
) -> Result<Json<()>, AppError> {
    auth.require_permission(PERM_PLOTS_TRANSACTIONS_CANCEL)?;

    let mut tx = state.db.begin().await?;

    let org_ok: bool = sqlx::query_scalar(
        "select exists(select 1 from plot_sales where id = $1 and organization_id = $2)",
    )
    .bind(sale_id)
    .bind(auth.organization_id)
    .fetch_one(&mut *tx)
    .await?;
    if !org_ok {
        return Err(AppError::NotFound);
    }

    let reason = input.reason.as_deref().map(str::trim).filter(|s| !s.is_empty());

    let updated: Option<Uuid> = sqlx::query_scalar(
        r#"
        update plot_sales set status = 'cancelled', status_reason = $1, status_changed_at = now(), status_changed_by = $2
        where id = $3 and status = 'active'
        returning id
        "#,
    )
    .bind(reason)
    .bind(auth.user_id)
    .bind(sale_id)
    .fetch_optional(&mut *tx)
    .await?;
    if updated.is_none() {
        return Err(AppError::conflict("This sale has already been cancelled or repossessed."));
    }

    sqlx::query("update sale_plots set status = 'cancelled' where sale_id = $1")
        .bind(sale_id)
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        r#"update plots set status = 'cancelled', assigned_customer_id = null
           where id in (select plot_id from sale_plots where sale_id = $1)"#,
    )
    .bind(sale_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query("update plot_loan_accounts set status = 'cancelled' where sale_id = $1")
        .bind(sale_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(Json(()))
}

/// Default-driven repossession — requires an active (not already fully
/// paid/closed/cancelled/already-repossessed) loan account on the
/// sale, since repossessing only makes sense when money is still owed.
/// Every plot on the sale moves to `Blocked` (not `Cancelled` — a
/// deliberately distinct terminal state, signalling "pending review"
/// rather than "cleanly available again"); the loan account moves to
/// `RepossessedOrReallocated`, its `outstanding_balance` left as a
/// historical record of what the customer still owed. Either terminal
/// plot state is freed back to `Available` the same way, via
/// `routes/projects.rs::reallocate_plot`.
async fn repossess_sale(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(sale_id): Path<Uuid>,
    Json(input): Json<RepossessSaleInput>,
) -> Result<Json<()>, AppError> {
    auth.require_permission(PERM_PLOTS_TRANSACTIONS_CANCEL)?;

    let mut tx = state.db.begin().await?;

    let org_ok: bool = sqlx::query_scalar(
        "select exists(select 1 from plot_sales where id = $1 and organization_id = $2)",
    )
    .bind(sale_id)
    .bind(auth.organization_id)
    .fetch_one(&mut *tx)
    .await?;
    if !org_ok {
        return Err(AppError::NotFound);
    }

    let loan_status: Option<String> = sqlx::query_scalar(
        "select status from plot_loan_accounts where sale_id = $1",
    )
    .bind(sale_id)
    .fetch_optional(&mut *tx)
    .await?;
    match loan_status.as_deref() {
        None => {
            return Err(AppError::bad_request(
                "This sale has no loan account — repossession only applies to Lipa Pole Pole sales that still owe a balance.",
            ));
        }
        Some("fully_paid") | Some("cancelled") | Some("closed") | Some("repossessed_or_reallocated") => {
            return Err(AppError::bad_request(
                "This loan account is already settled or closed — there's nothing to repossess.",
            ));
        }
        _ => {}
    }

    let reason = input.reason.as_deref().map(str::trim).filter(|s| !s.is_empty());

    let updated: Option<Uuid> = sqlx::query_scalar(
        r#"
        update plot_sales set status = 'repossessed', status_reason = $1, status_changed_at = now(), status_changed_by = $2
        where id = $3 and status = 'active'
        returning id
        "#,
    )
    .bind(reason)
    .bind(auth.user_id)
    .bind(sale_id)
    .fetch_optional(&mut *tx)
    .await?;
    if updated.is_none() {
        return Err(AppError::conflict("This sale has already been cancelled or repossessed."));
    }

    sqlx::query("update sale_plots set status = 'repossessed' where sale_id = $1")
        .bind(sale_id)
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        r#"update plots set status = 'blocked', assigned_customer_id = null
           where id in (select plot_id from sale_plots where sale_id = $1)"#,
    )
    .bind(sale_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query("update plot_loan_accounts set status = 'repossessed_or_reallocated' where sale_id = $1")
        .bind(sale_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(Json(()))
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
    auth.require_permission(PERM_PLOTS_TRANSACTIONS_BULK_IMPORT)?;

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

    sqlx::query("insert into sale_plots (sale_id, plot_id) values ($1, $2)")
        .bind(sale_id)
        .bind(plot_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| match &e {
            sqlx::Error::Database(db_err)
                if db_err.constraint() == Some("sale_plots_plot_uidx") =>
            {
                // Reachable even though the `plot_sales` insert above
                // just succeeded: that only enforces uniqueness on
                // `plot_sales.plot_id` (the *primary* plot column), so
                // a plot that was only ever an *additional* plot on
                // some other sale passes that check but still
                // collides here.
                AppError::conflict("This plot already has an active sale.")
            }
            _ => AppError::from(e),
        })?;
    sqlx::query("insert into sale_customers (sale_id, customer_id, role) values ($1, $2, 'primary')")
        .bind(sale_id)
        .bind(customer_id)
        .execute(&mut *tx)
        .await?;

    // Matches `execute_sale`: a cash sale is paid in full the moment
    // it's recorded, live or imported, so it always lands on `Sold`
    // directly. A Lipa Pole Pole import additionally goes straight to
    // `Sold` if the imported `amount_paid` already covers the full
    // price — a live LPP sale can't start that way (no payment has
    // happened yet), but a historical one might already be finished.
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

        let loan_account_id: Uuid = sqlx::query_scalar(
            r#"
            insert into plot_loan_accounts
                (account_number, sale_id, principal, interest_rate, deposit_required, deposit_paid,
                 instalment_amount, repayment_frequency_days, start_date, status, amount_paid, outstanding_balance)
            values ($1, $2, $3, $4, $5, $6, $7, 30, $8, $9, $10, $11)
            returning id
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
        .fetch_one(&mut *tx)
        .await?;

        insert_repayment_schedule(&mut *tx, loan_account_id, deposit_required, instalment_amount, 30, input.sale_date)
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
