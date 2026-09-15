use axum::extract::Path;
use axum::{extract::State, routing::get, Json, Router};
use chrono::{DateTime, Utc};
use domain::{
    Customer, CustomerDetail, CustomerSaleView, CustomerSummary, CreateCustomerInput,
};
use uuid::Uuid;

use crate::error::AppError;
use crate::extractors::AuthUser;
use crate::pg_enum::from_pg;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/v1/customers", get(list_customers).post(create_customer))
        .route("/api/v1/customers/:id", get(get_customer))
}

#[derive(sqlx::FromRow)]
struct CustomerRow {
    id: Uuid,
    organization_id: Uuid,
    full_name: String,
    email: Option<String>,
    phone: Option<String>,
    id_number: Option<String>,
    assigned_agent_id: Option<Uuid>,
    created_at: DateTime<Utc>,
}

impl From<CustomerRow> for Customer {
    fn from(r: CustomerRow) -> Self {
        Customer {
            id: r.id,
            organization_id: r.organization_id,
            full_name: r.full_name,
            email: r.email,
            phone: r.phone,
            id_number: r.id_number,
            assigned_agent_id: r.assigned_agent_id,
            created_at: r.created_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct CustomerSummaryRow {
    id: Uuid,
    organization_id: Uuid,
    full_name: String,
    email: Option<String>,
    phone: Option<String>,
    id_number: Option<String>,
    assigned_agent_id: Option<Uuid>,
    created_at: DateTime<Utc>,
    plots_owned: i64,
}

async fn list_customers(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<CustomerSummary>>, AppError> {
    let rows: Vec<CustomerSummaryRow> = sqlx::query_as(
        r#"
        select c.id, c.organization_id, c.full_name, c.email, c.phone, c.id_number, c.assigned_agent_id, c.created_at,
            count(pl.id) as plots_owned
        from customers c
        left join plots pl on pl.assigned_customer_id = c.id
        where c.organization_id = $1
        group by c.id
        order by c.created_at
        "#,
    )
    .bind(auth.organization_id)
    .fetch_all(&state.db)
    .await?;

    let summaries = rows
        .into_iter()
        .map(|r| CustomerSummary {
            customer: Customer {
                id: r.id,
                organization_id: r.organization_id,
                full_name: r.full_name,
                email: r.email,
                phone: r.phone,
                id_number: r.id_number,
                assigned_agent_id: r.assigned_agent_id,
                created_at: r.created_at,
            },
            plots_owned: r.plots_owned as u32,
        })
        .collect();

    Ok(Json(summaries))
}

async fn create_customer(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<CreateCustomerInput>,
) -> Result<Json<Customer>, AppError> {
    let full_name = input.full_name.trim();
    if full_name.is_empty() {
        return Err(AppError::bad_request("Enter the customer's name."));
    }

    let id_number = input.id_number.as_deref().map(str::trim).filter(|s| !s.is_empty());
    if let Some(id_number) = id_number {
        let duplicate: bool = sqlx::query_scalar(
            "select exists(select 1 from customers where organization_id = $1 and id_number = $2)",
        )
        .bind(auth.organization_id)
        .bind(id_number)
        .fetch_one(&state.db)
        .await?;
        if duplicate {
            return Err(AppError::conflict(
                "That ID/passport number is already registered.",
            ));
        }
    }

    let email = input.email.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let phone = input.phone.as_deref().map(str::trim).filter(|s| !s.is_empty());

    let row: CustomerRow = sqlx::query_as(
        r#"
        insert into customers (organization_id, full_name, email, phone, id_number, assigned_agent_id)
        values ($1, $2, $3, $4, $5, $6)
        returning id, organization_id, full_name, email, phone, id_number, assigned_agent_id, created_at
        "#,
    )
    .bind(auth.organization_id)
    .bind(full_name)
    .bind(email)
    .bind(phone)
    .bind(id_number)
    .bind(auth.user_id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(row.into()))
}

#[derive(sqlx::FromRow)]
struct CustomerSaleRow {
    sale_id: Uuid,
    plot_id: Uuid,
    project_id: Uuid,
    plot_number: String,
    project_name: String,
    payment_mode: String,
    agreed_price: rust_decimal::Decimal,
    plot_status: String,
    loan_account_id: Option<Uuid>,
}

async fn get_customer(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<CustomerDetail>, AppError> {
    let customer_row: Option<CustomerRow> = sqlx::query_as(
        r#"
        select id, organization_id, full_name, email, phone, id_number, assigned_agent_id, created_at
        from customers where id = $1 and organization_id = $2
        "#,
    )
    .bind(id)
    .bind(auth.organization_id)
    .fetch_optional(&state.db)
    .await?;
    let customer_row = customer_row.ok_or(AppError::NotFound)?;

    let sale_rows: Vec<CustomerSaleRow> = sqlx::query_as(
        r#"
        select ps.id as sale_id, pl.id as plot_id, pr.id as project_id, pl.plot_number,
            pr.name as project_name, ps.payment_mode, ps.agreed_price, pl.status as plot_status,
            pla.id as loan_account_id
        from plot_sales ps
        join plots pl on pl.id = ps.plot_id
        join projects pr on pr.id = pl.project_id
        left join plot_loan_accounts pla on pla.sale_id = ps.id
        where ps.customer_id = $1
        order by ps.created_at desc
        "#,
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    let sales = sale_rows
        .into_iter()
        .map(|r| {
            let status = from_pg("plots.status", &r.plot_status)?;
            let (label, color) = domain::plot_status_meta(status);
            Ok(CustomerSaleView {
                sale_id: r.sale_id,
                plot_id: r.plot_id,
                project_id: r.project_id,
                plot_number: r.plot_number,
                project_name: r.project_name,
                payment_mode: from_pg("plot_sales.payment_mode", &r.payment_mode)?,
                agreed_price: r.agreed_price,
                status_label: label.to_string(),
                status_color: color.to_string(),
                loan_account_id: r.loan_account_id,
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;

    Ok(Json(CustomerDetail {
        customer: customer_row.into(),
        sales,
    }))
}
