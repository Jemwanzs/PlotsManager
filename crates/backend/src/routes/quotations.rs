//! Quotations sit between a lead being interested and a committed sale
//! (`PlotSale`) — see docs/07's "quotations and offer letters" funnel
//! stage, which is otherwise unspecified (module docs on
//! `domain::Quotation` and `database/migrations/0007_quotations.sql`
//! cover the design decisions). `accept_quotation` reuses
//! `routes::sales::execute_sale` so a quotation-driven sale and a
//! directly-reserved sale go through exactly the same transaction logic.

use axum::extract::{Path, Query};
use axum::routing::post;
use axum::{extract::State, routing::get, Json, Router};
use chrono::{DateTime, NaiveDate, Utc};
use domain::{
    CreateQuotationInput, PaymentMode, Quotation, QuotationDetail, QuotationStatus,
    QuotationSummary,
};
use rust_decimal::Decimal;
use uuid::Uuid;

use super::sales::{execute_sale, ExecuteSaleParams};
use crate::error::AppError;
use crate::extractors::AuthUser;
use crate::pg_enum::{from_pg, to_pg};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/quotations", get(list_quotations).post(create_quotation))
        .route("/api/v1/quotations/:id", get(get_quotation))
        .route("/api/v1/quotations/:id/send", post(send_quotation))
        .route("/api/v1/quotations/:id/accept", post(accept_quotation))
        .route("/api/v1/quotations/:id/reject", post(reject_quotation))
}

fn is_expired(status: QuotationStatus, valid_until: NaiveDate) -> bool {
    status == QuotationStatus::Sent && valid_until < Utc::now().date_naive()
}

#[derive(sqlx::FromRow)]
struct QuotationSummaryRow {
    id: Uuid,
    organization_id: Uuid,
    plot_id: Uuid,
    customer_id: Uuid,
    agent_id: Option<Uuid>,
    payment_mode: String,
    quoted_price: Decimal,
    valid_until: NaiveDate,
    status: String,
    notes: Option<String>,
    converted_sale_id: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    plot_number: String,
    project_name: String,
    customer_name: String,
}

impl QuotationSummaryRow {
    fn into_domain(self) -> Result<QuotationSummary, AppError> {
        let status: QuotationStatus = from_pg("quotations.status", &self.status)?;
        let expired = is_expired(status, self.valid_until);
        let (label, color) = domain::quotation_status_meta(status, expired);
        Ok(QuotationSummary {
            quotation: Quotation {
                id: self.id,
                organization_id: self.organization_id,
                plot_id: self.plot_id,
                customer_id: self.customer_id,
                agent_id: self.agent_id,
                payment_mode: from_pg("quotations.payment_mode", &self.payment_mode)?,
                quoted_price: self.quoted_price,
                valid_until: self.valid_until,
                status,
                notes: self.notes,
                converted_sale_id: self.converted_sale_id,
                created_at: self.created_at,
                updated_at: self.updated_at,
            },
            plot_number: self.plot_number,
            project_name: self.project_name,
            customer_name: self.customer_name,
            status_label: label.to_string(),
            status_color: color.to_string(),
            is_expired: expired,
        })
    }
}

const QUOTATION_SUMMARY_QUERY: &str = r#"
    select q.id, q.organization_id, q.plot_id, q.customer_id, q.agent_id, q.payment_mode,
        q.quoted_price, q.valid_until, q.status, q.notes, q.converted_sale_id, q.created_at, q.updated_at,
        pl.plot_number, pr.name as project_name, c.full_name as customer_name
    from quotations q
    join plots pl on pl.id = q.plot_id
    join projects pr on pr.id = pl.project_id
    join customers c on c.id = q.customer_id
    where q.organization_id = $1
"#;

#[derive(serde::Deserialize)]
struct ListQuotationsQuery {
    customer_id: Option<Uuid>,
}

async fn list_quotations(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(params): Query<ListQuotationsQuery>,
) -> Result<Json<Vec<QuotationSummary>>, AppError> {
    let rows: Vec<QuotationSummaryRow> = sqlx::query_as(&format!(
        "{QUOTATION_SUMMARY_QUERY} and ($2::uuid is null or q.customer_id = $2) order by q.created_at desc"
    ))
    .bind(auth.organization_id)
    .bind(params.customer_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(QuotationSummaryRow::into_domain)
            .collect::<Result<Vec<_>, AppError>>()?,
    ))
}

async fn create_quotation(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<CreateQuotationInput>,
) -> Result<Json<Quotation>, AppError> {
    if input.quoted_price <= Decimal::ZERO {
        return Err(AppError::bad_request(
            "Enter a quoted price greater than zero.",
        ));
    }
    if input.valid_until < Utc::now().date_naive() {
        return Err(AppError::bad_request(
            "Choose a validity date that isn't in the past.",
        ));
    }

    let plot_ok: bool = sqlx::query_scalar(
        r#"select exists(
            select 1 from plots pl join projects p on p.id = pl.project_id
            where pl.id = $1 and p.organization_id = $2
        )"#,
    )
    .bind(input.plot_id)
    .bind(auth.organization_id)
    .fetch_one(&state.db)
    .await?;
    if !plot_ok {
        return Err(AppError::NotFound);
    }

    let customer_ok: bool = sqlx::query_scalar(
        "select exists(select 1 from customers where id = $1 and organization_id = $2)",
    )
    .bind(input.customer_id)
    .bind(auth.organization_id)
    .fetch_one(&state.db)
    .await?;
    if !customer_ok {
        return Err(AppError::bad_request("Choose a valid customer."));
    }

    let notes = input.notes.as_deref().map(str::trim).filter(|s| !s.is_empty());

    let row: QuotationRow = sqlx::query_as(
        r#"
        insert into quotations (organization_id, plot_id, customer_id, agent_id, payment_mode, quoted_price, valid_until, notes)
        values ($1, $2, $3, $4, $5, $6, $7, $8)
        returning id, organization_id, plot_id, customer_id, agent_id, payment_mode, quoted_price, valid_until, status, notes, converted_sale_id, created_at, updated_at
        "#,
    )
    .bind(auth.organization_id)
    .bind(input.plot_id)
    .bind(input.customer_id)
    .bind(auth.user_id)
    .bind(to_pg(&input.payment_mode))
    .bind(input.quoted_price)
    .bind(input.valid_until)
    .bind(notes)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(row.into_domain()?))
}

#[derive(sqlx::FromRow)]
struct QuotationRow {
    id: Uuid,
    organization_id: Uuid,
    plot_id: Uuid,
    customer_id: Uuid,
    agent_id: Option<Uuid>,
    payment_mode: String,
    quoted_price: Decimal,
    valid_until: NaiveDate,
    status: String,
    notes: Option<String>,
    converted_sale_id: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl QuotationRow {
    fn into_domain(self) -> Result<Quotation, AppError> {
        Ok(Quotation {
            id: self.id,
            organization_id: self.organization_id,
            plot_id: self.plot_id,
            customer_id: self.customer_id,
            agent_id: self.agent_id,
            payment_mode: from_pg("quotations.payment_mode", &self.payment_mode)?,
            quoted_price: self.quoted_price,
            valid_until: self.valid_until,
            status: from_pg("quotations.status", &self.status)?,
            notes: self.notes,
            converted_sale_id: self.converted_sale_id,
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
}

async fn get_quotation(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<QuotationDetail>, AppError> {
    // A dedicated row struct with its own detail-only columns, rather
    // than `#[sqlx(flatten)]`-ing QuotationSummaryRow — this codebase
    // avoids that attribute (uncertain support against the pinned sqlx
    // 0.7.4, see routes/customers.rs's comment).
    #[derive(sqlx::FromRow)]
    struct QuotationDetailRow {
        id: Uuid,
        organization_id: Uuid,
        plot_id: Uuid,
        customer_id: Uuid,
        agent_id: Option<Uuid>,
        payment_mode: String,
        quoted_price: Decimal,
        valid_until: NaiveDate,
        status: String,
        notes: Option<String>,
        converted_sale_id: Option<Uuid>,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
        plot_number: String,
        project_id: Uuid,
        project_name: String,
        asking_price: Decimal,
        minimum_price: Decimal,
        customer_name: String,
    }

    let row: Option<QuotationDetailRow> = sqlx::query_as(
        r#"
        select q.id, q.organization_id, q.plot_id, q.customer_id, q.agent_id, q.payment_mode,
            q.quoted_price, q.valid_until, q.status, q.notes, q.converted_sale_id, q.created_at, q.updated_at,
            pl.plot_number, pr.id as project_id, pr.name as project_name,
            pl.asking_price, pl.minimum_price, c.full_name as customer_name
        from quotations q
        join plots pl on pl.id = q.plot_id
        join projects pr on pr.id = pl.project_id
        join customers c on c.id = q.customer_id
        where q.id = $1 and q.organization_id = $2
        "#,
    )
    .bind(id)
    .bind(auth.organization_id)
    .fetch_optional(&state.db)
    .await?;
    let row = row.ok_or(AppError::NotFound)?;

    let status: QuotationStatus = from_pg("quotations.status", &row.status)?;
    let expired = is_expired(status, row.valid_until);
    let (label, color) = domain::quotation_status_meta(status, expired);

    Ok(Json(QuotationDetail {
        quotation: Quotation {
            id: row.id,
            organization_id: row.organization_id,
            plot_id: row.plot_id,
            customer_id: row.customer_id,
            agent_id: row.agent_id,
            payment_mode: from_pg("quotations.payment_mode", &row.payment_mode)?,
            quoted_price: row.quoted_price,
            valid_until: row.valid_until,
            status,
            notes: row.notes,
            converted_sale_id: row.converted_sale_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
        },
        plot_id: row.plot_id,
        plot_number: row.plot_number,
        project_id: row.project_id,
        project_name: row.project_name,
        asking_price: row.asking_price,
        minimum_price: row.minimum_price,
        customer_id: row.customer_id,
        customer_name: row.customer_name,
        status_label: label.to_string(),
        status_color: color.to_string(),
        is_expired: expired,
        below_minimum_price: row.quoted_price < row.minimum_price,
    }))
}

async fn send_quotation(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Quotation>, AppError> {
    transition(&state, auth.organization_id, id, QuotationStatus::Draft, QuotationStatus::Sent).await
}

async fn reject_quotation(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Quotation>, AppError> {
    transition(&state, auth.organization_id, id, QuotationStatus::Sent, QuotationStatus::Rejected).await
}

async fn transition(
    state: &AppState,
    organization_id: Uuid,
    id: Uuid,
    from: QuotationStatus,
    to: QuotationStatus,
) -> Result<Json<Quotation>, AppError> {
    let row: Option<QuotationRow> = sqlx::query_as(
        r#"
        update quotations set status = $1, updated_at = now()
        where id = $2 and organization_id = $3 and status = $4
        returning id, organization_id, plot_id, customer_id, agent_id, payment_mode, quoted_price, valid_until, status, notes, converted_sale_id, created_at, updated_at
        "#,
    )
    .bind(to_pg(&to))
    .bind(id)
    .bind(organization_id)
    .bind(to_pg(&from))
    .fetch_optional(&state.db)
    .await?;

    match row {
        Some(row) => Ok(Json(row.into_domain()?)),
        None => {
            // Distinguish "doesn't exist" from "exists but in the wrong
            // state" so the UI can show something more useful than a
            // generic 404.
            let exists: bool =
                sqlx::query_scalar("select exists(select 1 from quotations where id = $1 and organization_id = $2)")
                    .bind(id)
                    .bind(organization_id)
                    .fetch_one(&state.db)
                    .await?;
            if exists {
                Err(AppError::conflict(
                    "This quotation isn't in the right state for that action anymore.",
                ))
            } else {
                Err(AppError::NotFound)
            }
        }
    }
}

async fn accept_quotation(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Quotation>, AppError> {
    let mut tx = state.db.begin().await?;

    let existing: Option<(String, Uuid, Uuid, String, Decimal, Option<Uuid>)> = sqlx::query_as(
        r#"select status, plot_id, customer_id, payment_mode, quoted_price, agent_id
           from quotations where id = $1 and organization_id = $2 for update"#,
    )
    .bind(id)
    .bind(auth.organization_id)
    .fetch_optional(&mut *tx)
    .await?;
    let (status, plot_id, customer_id, payment_mode, quoted_price, agent_id) =
        existing.ok_or(AppError::NotFound)?;
    let status: QuotationStatus = from_pg("quotations.status", &status)?;
    if status != QuotationStatus::Sent {
        return Err(AppError::conflict(
            "Only a sent quotation can be accepted.",
        ));
    }
    let payment_mode: PaymentMode = from_pg("quotations.payment_mode", &payment_mode)?;

    let sale = execute_sale(
        &mut tx,
        auth.organization_id,
        ExecuteSaleParams {
            plot_id,
            customer_id,
            agent_id: agent_id.unwrap_or(auth.user_id),
            payment_mode,
            agreed_price: quoted_price,
        },
    )
    .await?;

    let row: QuotationRow = sqlx::query_as(
        r#"
        update quotations set status = 'accepted', converted_sale_id = $1, updated_at = now()
        where id = $2
        returning id, organization_id, plot_id, customer_id, agent_id, payment_mode, quoted_price, valid_until, status, notes, converted_sale_id, created_at, updated_at
        "#,
    )
    .bind(sale.id)
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(row.into_domain()?))
}
