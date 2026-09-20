//! A deliberately small slice of docs/09's approval-workflow engine —
//! see `database/migrations/0008_approvals.sql`'s module comment for
//! the scope decisions (one trigger, one step, no approver-role
//! scoping). `gate_price` is the actual gate, called from
//! `routes/sales.rs::create_sale` and
//! `routes/quotations.rs::accept_quotation`; the rest of this module is
//! the CRUD an approver needs to see and decide on pending requests.

use axum::extract::{Path, Query};
use axum::routing::{get, post};
use axum::{extract::State, Json, Router};
use chrono::{DateTime, Utc};
use domain::{
    ApprovalRequest, ApprovalRequestSummary, ApprovalStatus, DecideApprovalInput, PaymentMode,
    PERM_APPROVE_TRANSACTIONS,
};
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::extractors::AuthUser;
use crate::pg_enum::{from_pg, to_pg};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/approvals", get(list_approvals))
        .route("/api/v1/approvals/:id/approve", post(approve))
        .route("/api/v1/approvals/:id/reject", post(reject))
}

/// Checks whether `agreed_price` needs sign-off before the sale it's
/// part of may proceed, and if so, records that.
///
/// Runs against the pool directly, **not** the caller's sale
/// transaction: a newly-recorded pending request must survive even
/// though the sale it's gating does not happen this call — if this ran
/// inside that transaction, returning `Err` to abort the sale would
/// roll the pending row back too, silently losing the request.
///
/// Returns:
/// - `Ok(None)` — `agreed_price` is at or above the plot's
///   `minimum_price`; no gate needed, caller proceeds immediately.
/// - `Ok(Some(approval_id))` — an approved, not-yet-consumed request
///   already covers this exact (plot, customer, payment mode, price);
///   caller proceeds and must record `resulting_sale_id` on it once the
///   sale exists, inside its own transaction (see `sales.rs::create_sale`).
/// - `Err(Conflict)` — still pending (freshly recorded, or already was).
pub(crate) async fn gate_price(
    db: &PgPool,
    organization_id: Uuid,
    requested_by: Uuid,
    plot_id: Uuid,
    customer_id: Uuid,
    agent_id: Option<Uuid>,
    payment_mode: PaymentMode,
    agreed_price: Decimal,
    quotation_id: Option<Uuid>,
) -> Result<Option<Uuid>, AppError> {
    let minimum_price: Decimal = sqlx::query_scalar("select minimum_price from plots where id = $1")
        .bind(plot_id)
        .fetch_one(db)
        .await?;

    if agreed_price >= minimum_price {
        return Ok(None);
    }

    let payment_mode_pg = to_pg(&payment_mode);

    let approved: Option<Uuid> = sqlx::query_scalar(
        r#"select id from approval_requests
           where organization_id = $1 and plot_id = $2 and customer_id = $3
             and payment_mode = $4 and agreed_price = $5
             and status = 'approved' and resulting_sale_id is null
           order by decided_at desc limit 1"#,
    )
    .bind(organization_id)
    .bind(plot_id)
    .bind(customer_id)
    .bind(&payment_mode_pg)
    .bind(agreed_price)
    .fetch_optional(db)
    .await?;
    if let Some(id) = approved {
        return Ok(Some(id));
    }

    let already_pending: bool = sqlx::query_scalar(
        r#"select exists(select 1 from approval_requests
           where organization_id = $1 and plot_id = $2 and customer_id = $3
             and payment_mode = $4 and agreed_price = $5 and status = 'pending')"#,
    )
    .bind(organization_id)
    .bind(plot_id)
    .bind(customer_id)
    .bind(&payment_mode_pg)
    .bind(agreed_price)
    .fetch_one(db)
    .await?;
    if already_pending {
        return Err(AppError::conflict(
            "This price is below the plot's minimum and is still awaiting approval.",
        ));
    }

    let reason = format!(
        "Price {agreed_price} is below this plot's minimum of {minimum_price}."
    );
    sqlx::query(
        r#"insert into approval_requests
            (organization_id, plot_id, customer_id, agent_id, payment_mode, agreed_price,
             minimum_price, quotation_id, requested_by, reason)
           values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"#,
    )
    .bind(organization_id)
    .bind(plot_id)
    .bind(customer_id)
    .bind(agent_id)
    .bind(&payment_mode_pg)
    .bind(agreed_price)
    .bind(minimum_price)
    .bind(quotation_id)
    .bind(requested_by)
    .bind(reason)
    .execute(db)
    .await?;

    Err(AppError::conflict(
        "This price is below the plot's minimum. A request has been sent for approval — try again once it's approved.",
    ))
}

const APPROVAL_SUMMARY_QUERY: &str = r#"
    select a.id, a.organization_id, a.plot_id, a.customer_id, a.agent_id, a.payment_mode,
        a.agreed_price, a.minimum_price, a.quotation_id, a.requested_by, a.reason, a.status,
        a.decided_by, a.decided_at, a.decision_notes, a.resulting_sale_id, a.created_at,
        pl.plot_number, pr.name as project_name, c.full_name as customer_name,
        req.full_name as requested_by_name, dec.full_name as decided_by_name
    from approval_requests a
    join plots pl on pl.id = a.plot_id
    join projects pr on pr.id = pl.project_id
    join customers c on c.id = a.customer_id
    join users req on req.id = a.requested_by
    left join users dec on dec.id = a.decided_by
    where a.organization_id = $1
"#;

#[derive(sqlx::FromRow)]
struct ApprovalSummaryRow {
    id: Uuid,
    organization_id: Uuid,
    plot_id: Uuid,
    customer_id: Uuid,
    agent_id: Option<Uuid>,
    payment_mode: String,
    agreed_price: Decimal,
    minimum_price: Decimal,
    quotation_id: Option<Uuid>,
    requested_by: Uuid,
    reason: String,
    status: String,
    decided_by: Option<Uuid>,
    decided_at: Option<DateTime<Utc>>,
    decision_notes: Option<String>,
    resulting_sale_id: Option<Uuid>,
    created_at: DateTime<Utc>,
    plot_number: String,
    project_name: String,
    customer_name: String,
    requested_by_name: String,
    decided_by_name: Option<String>,
}

impl ApprovalSummaryRow {
    fn into_domain(self) -> Result<ApprovalRequestSummary, AppError> {
        let status: ApprovalStatus = from_pg("approval_requests.status", &self.status)?;
        let (label, color) = domain::approval_status_meta(status);
        Ok(ApprovalRequestSummary {
            request: ApprovalRequest {
                id: self.id,
                organization_id: self.organization_id,
                plot_id: self.plot_id,
                customer_id: self.customer_id,
                agent_id: self.agent_id,
                payment_mode: from_pg("approval_requests.payment_mode", &self.payment_mode)?,
                agreed_price: self.agreed_price,
                minimum_price: self.minimum_price,
                quotation_id: self.quotation_id,
                requested_by: self.requested_by,
                reason: self.reason,
                status,
                decided_by: self.decided_by,
                decided_at: self.decided_at,
                decision_notes: self.decision_notes,
                resulting_sale_id: self.resulting_sale_id,
                created_at: self.created_at,
            },
            plot_number: self.plot_number,
            project_name: self.project_name,
            customer_name: self.customer_name,
            requested_by_name: self.requested_by_name,
            decided_by_name: self.decided_by_name,
            status_label: label.to_string(),
            status_color: color.to_string(),
        })
    }
}

#[derive(serde::Deserialize)]
struct ListApprovalsQuery {
    status: Option<String>,
}

async fn list_approvals(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(params): Query<ListApprovalsQuery>,
) -> Result<Json<Vec<ApprovalRequestSummary>>, AppError> {
    let status_filter = params.status.filter(|s| !s.is_empty());
    let rows: Vec<ApprovalSummaryRow> = sqlx::query_as(&format!(
        "{APPROVAL_SUMMARY_QUERY} and ($2::text is null or a.status = $2) order by a.created_at desc"
    ))
    .bind(auth.organization_id)
    .bind(status_filter)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(ApprovalSummaryRow::into_domain)
            .collect::<Result<Vec<_>, AppError>>()?,
    ))
}

async fn approve(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<DecideApprovalInput>,
) -> Result<Json<ApprovalRequestSummary>, AppError> {
    decide(&state, auth, id, ApprovalStatus::Approved, input).await
}

async fn reject(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<DecideApprovalInput>,
) -> Result<Json<ApprovalRequestSummary>, AppError> {
    decide(&state, auth, id, ApprovalStatus::Rejected, input).await
}

async fn decide(
    state: &AppState,
    auth: AuthUser,
    id: Uuid,
    to: ApprovalStatus,
    input: DecideApprovalInput,
) -> Result<Json<ApprovalRequestSummary>, AppError> {
    auth.require_permission(PERM_APPROVE_TRANSACTIONS)?;

    let requested_by: Option<Uuid> = sqlx::query_scalar(
        "select requested_by from approval_requests where id = $1 and organization_id = $2 and status = 'pending'",
    )
    .bind(id)
    .bind(auth.organization_id)
    .fetch_optional(&state.db)
    .await?;

    let requested_by = match requested_by {
        Some(requested_by) => requested_by,
        None => {
            let exists: bool = sqlx::query_scalar(
                "select exists(select 1 from approval_requests where id = $1 and organization_id = $2)",
            )
            .bind(id)
            .bind(auth.organization_id)
            .fetch_one(&state.db)
            .await?;
            return Err(if exists {
                AppError::conflict("This request has already been decided.")
            } else {
                AppError::NotFound
            });
        }
    };

    // Separation of duties: the one meaningful control available while
    // every user is still an unrestricted 'Admin' (see this migration's
    // module comment) is that the requester can't also be the approver
    // — *if* someone else could plausibly decide it. Most orgs today
    // have exactly one active user (no invite flow exists yet — see
    // docs/14's roadmap), and blocking self-decision unconditionally
    // would leave every below-minimum sale stuck forever for them, not
    // safer. So the rule only bites once a second decider actually
    // exists to apply it to.
    if requested_by == auth.user_id {
        let other_users: bool = sqlx::query_scalar(
            "select exists(select 1 from users where organization_id = $1 and is_active and id != $2)",
        )
        .bind(auth.organization_id)
        .bind(auth.user_id)
        .fetch_one(&state.db)
        .await?;
        if other_users {
            return Err(AppError::forbidden(
                "You can't decide on a request you submitted yourself — ask another admin to review it.",
            ));
        }
    }

    let notes = input.notes.as_deref().map(str::trim).filter(|s| !s.is_empty());

    let updated: bool = sqlx::query_scalar(
        r#"update approval_requests
           set status = $1, decided_by = $2, decided_at = now(), decision_notes = $3
           where id = $4 and organization_id = $5 and status = 'pending'
           returning true"#,
    )
    .bind(to_pg(&to))
    .bind(auth.user_id)
    .bind(notes)
    .bind(id)
    .bind(auth.organization_id)
    .fetch_optional(&state.db)
    .await?
    .unwrap_or(false);
    if !updated {
        return Err(AppError::conflict("This request has already been decided."));
    }

    let row: ApprovalSummaryRow = sqlx::query_as(&format!("{APPROVAL_SUMMARY_QUERY} and a.id = $2"))
        .bind(auth.organization_id)
        .bind(id)
        .fetch_one(&state.db)
        .await?;

    Ok(Json(row.into_domain()?))
}
