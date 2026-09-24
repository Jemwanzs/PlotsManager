use axum::extract::Path;
use axum::routing::post;
use axum::{extract::State, routing::get, Json, Router};
use chrono::{DateTime, NaiveDate, Utc};
use domain::{
    BulkImportResult, BulkImportRowError, Customer, CustomerDetail, CustomerSaleView,
    CustomerSummary, CreateCustomerInput, UpdateCustomerInput, UpdateLeadInput,
    PERM_CUSTOMERS_BULK_IMPORT, PERM_CUSTOMERS_CREATE, PERM_CUSTOMERS_EDIT,
    PERM_CUSTOMERS_LEADS_UPDATE,
};
use uuid::Uuid;

use crate::error::AppError;
use crate::extractors::AuthUser;
use crate::pg_enum::{from_pg, to_pg};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/customers", get(list_customers).post(create_customer))
        .route("/api/v1/customers/bulk", post(bulk_create_customers))
        .route("/api/v1/customers/:id", get(get_customer).put(update_customer))
        .route("/api/v1/customers/:id/stage", post(update_lead))
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
    stage: String,
    source: Option<String>,
    next_follow_up_at: Option<NaiveDate>,
    notes: Option<String>,
    created_at: DateTime<Utc>,
    title: Option<String>,
    customer_type: String,
    kra_pin: Option<String>,
    postal_address: Option<String>,
    city: Option<String>,
    physical_address: Option<String>,
    legacy_customer_number: Option<String>,
    next_of_kin_name: Option<String>,
    next_of_kin_relationship: Option<String>,
    next_of_kin_mobile: Option<String>,
    next_of_kin_id_number: Option<String>,
    next_of_kin_address: Option<String>,
}

impl CustomerRow {
    fn into_domain(self) -> Result<Customer, AppError> {
        Ok(Customer {
            id: self.id,
            organization_id: self.organization_id,
            full_name: self.full_name,
            email: self.email,
            phone: self.phone,
            id_number: self.id_number,
            assigned_agent_id: self.assigned_agent_id,
            stage: from_pg("customers.stage", &self.stage)?,
            source: self.source,
            next_follow_up_at: self.next_follow_up_at,
            notes: self.notes,
            created_at: self.created_at,
            title: self.title,
            customer_type: from_pg("customers.customer_type", &self.customer_type)?,
            kra_pin: self.kra_pin,
            postal_address: self.postal_address,
            city: self.city,
            physical_address: self.physical_address,
            legacy_customer_number: self.legacy_customer_number,
            next_of_kin_name: self.next_of_kin_name,
            next_of_kin_relationship: self.next_of_kin_relationship,
            next_of_kin_mobile: self.next_of_kin_mobile,
            next_of_kin_id_number: self.next_of_kin_id_number,
            next_of_kin_address: self.next_of_kin_address,
        })
    }
}

const CUSTOMER_COLUMNS: &str = "id, organization_id, full_name, email, phone, id_number, \
    assigned_agent_id, stage, source, next_follow_up_at, notes, created_at, title, customer_type, \
    kra_pin, postal_address, city, physical_address, legacy_customer_number, next_of_kin_name, \
    next_of_kin_relationship, next_of_kin_mobile, next_of_kin_id_number, next_of_kin_address";

// Manually flattened rather than `#[sqlx(flatten)]` on a nested
// `CustomerRow` — that attribute's support was uncertain against the
// pinned sqlx 0.7.4 when this was last checked (see routes/loan_accounts.rs
// history), so every *SummaryRow/*Row pair in this codebase spells out
// the columns twice instead of risking it.
#[derive(sqlx::FromRow)]
struct CustomerSummaryRow {
    id: Uuid,
    organization_id: Uuid,
    full_name: String,
    email: Option<String>,
    phone: Option<String>,
    id_number: Option<String>,
    assigned_agent_id: Option<Uuid>,
    stage: String,
    source: Option<String>,
    next_follow_up_at: Option<NaiveDate>,
    notes: Option<String>,
    created_at: DateTime<Utc>,
    title: Option<String>,
    customer_type: String,
    kra_pin: Option<String>,
    postal_address: Option<String>,
    city: Option<String>,
    physical_address: Option<String>,
    legacy_customer_number: Option<String>,
    next_of_kin_name: Option<String>,
    next_of_kin_relationship: Option<String>,
    next_of_kin_mobile: Option<String>,
    next_of_kin_id_number: Option<String>,
    next_of_kin_address: Option<String>,
    plots_owned: i64,
}

impl CustomerSummaryRow {
    fn into_domain(self) -> Result<CustomerSummary, AppError> {
        Ok(CustomerSummary {
            customer: Customer {
                id: self.id,
                organization_id: self.organization_id,
                full_name: self.full_name,
                email: self.email,
                phone: self.phone,
                id_number: self.id_number,
                assigned_agent_id: self.assigned_agent_id,
                stage: from_pg("customers.stage", &self.stage)?,
                source: self.source,
                next_follow_up_at: self.next_follow_up_at,
                notes: self.notes,
                created_at: self.created_at,
                title: self.title,
                customer_type: from_pg("customers.customer_type", &self.customer_type)?,
                kra_pin: self.kra_pin,
                postal_address: self.postal_address,
                city: self.city,
                physical_address: self.physical_address,
                legacy_customer_number: self.legacy_customer_number,
                next_of_kin_name: self.next_of_kin_name,
                next_of_kin_relationship: self.next_of_kin_relationship,
                next_of_kin_mobile: self.next_of_kin_mobile,
                next_of_kin_id_number: self.next_of_kin_id_number,
                next_of_kin_address: self.next_of_kin_address,
            },
            plots_owned: self.plots_owned as u32,
        })
    }
}

async fn list_customers(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<CustomerSummary>>, AppError> {
    let rows: Vec<CustomerSummaryRow> = sqlx::query_as(
        r#"
        select c.id, c.organization_id, c.full_name, c.email, c.phone, c.id_number,
            c.assigned_agent_id, c.stage, c.source, c.next_follow_up_at, c.notes, c.created_at,
            c.title, c.customer_type, c.kra_pin, c.postal_address, c.city, c.physical_address,
            c.legacy_customer_number, c.next_of_kin_name, c.next_of_kin_relationship,
            c.next_of_kin_mobile, c.next_of_kin_id_number, c.next_of_kin_address,
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
        .map(CustomerSummaryRow::into_domain)
        .collect::<Result<Vec<_>, AppError>>()?;

    Ok(Json(summaries))
}

async fn create_customer(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<CreateCustomerInput>,
) -> Result<Json<Customer>, AppError> {
    auth.require_permission(PERM_CUSTOMERS_CREATE)?;
    Ok(Json(insert_customer(&state, auth.organization_id, auth.user_id, &input).await?))
}

/// The single-row validation-and-insert `create_customer` and
/// `bulk_create_customers` both go through, so a bulk CSV import
/// can't drift from what adding one customer by hand enforces.
async fn insert_customer(
    state: &AppState,
    organization_id: Uuid,
    agent_id: Uuid,
    input: &CreateCustomerInput,
) -> Result<Customer, AppError> {
    let full_name = input.full_name.trim();
    if full_name.is_empty() {
        return Err(AppError::bad_request("Enter the customer's name."));
    }

    let id_number = input.id_number.as_deref().map(str::trim).filter(|s| !s.is_empty());
    if let Some(id_number) = id_number {
        let duplicate: bool = sqlx::query_scalar(
            "select exists(select 1 from customers where organization_id = $1 and id_number = $2)",
        )
        .bind(organization_id)
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
    let source = input.source.as_deref().map(str::trim).filter(|s| !s.is_empty());

    let row: CustomerRow = sqlx::query_as(&format!(
        r#"
        insert into customers (organization_id, full_name, email, phone, id_number, assigned_agent_id, source)
        values ($1, $2, $3, $4, $5, $6, $7)
        returning {CUSTOMER_COLUMNS}
        "#
    ))
    .bind(organization_id)
    .bind(full_name)
    .bind(email)
    .bind(phone)
    .bind(id_number)
    .bind(agent_id)
    .bind(source)
    .fetch_one(&state.db)
    .await?;

    row.into_domain()
}

/// Best-effort bulk import (see `domain::BulkImportResult`'s module
/// docs) — a CSV upload during tenant onboarding, parsed to
/// `CreateCustomerInput` rows client-side and posted here as JSON.
async fn bulk_create_customers(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(inputs): Json<Vec<CreateCustomerInput>>,
) -> Result<Json<BulkImportResult>, AppError> {
    auth.require_permission(PERM_CUSTOMERS_BULK_IMPORT)?;

    let mut created = 0u32;
    let mut errors = Vec::new();
    for (idx, input) in inputs.iter().enumerate() {
        match insert_customer(&state, auth.organization_id, auth.user_id, input).await {
            Ok(_) => created += 1,
            Err(e) => errors.push(BulkImportRowError {
                row: idx as u32 + 1,
                message: e.client_message(),
            }),
        }
    }

    Ok(Json(BulkImportResult { created, errors }))
}

/// Editing a customer's own profile — see `PERM_CUSTOMERS_EDIT`'s doc
/// comment for why this didn't exist until now. Applies the same
/// full-name/ID-number validation `insert_customer` does, since both
/// are the same record just at different points in its life.
async fn update_customer(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateCustomerInput>,
) -> Result<Json<Customer>, AppError> {
    auth.require_permission(PERM_CUSTOMERS_EDIT)?;

    let full_name = input.full_name.trim();
    if full_name.is_empty() {
        return Err(AppError::bad_request("Enter the customer's name."));
    }

    let id_number = input.id_number.as_deref().map(str::trim).filter(|s| !s.is_empty());
    if let Some(id_number) = id_number {
        let duplicate: bool = sqlx::query_scalar(
            "select exists(select 1 from customers where organization_id = $1 and id_number = $2 and id <> $3)",
        )
        .bind(auth.organization_id)
        .bind(id_number)
        .bind(id)
        .fetch_one(&state.db)
        .await?;
        if duplicate {
            return Err(AppError::conflict(
                "That ID/passport number is already registered.",
            ));
        }
    }

    let clean = |v: &Option<String>| v.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);

    let row: Option<CustomerRow> = sqlx::query_as(&format!(
        r#"
        update customers set
            full_name = $1, email = $2, phone = $3, id_number = $4, title = $5,
            customer_type = $6, kra_pin = $7, postal_address = $8, city = $9,
            physical_address = $10, legacy_customer_number = $11, next_of_kin_name = $12,
            next_of_kin_relationship = $13, next_of_kin_mobile = $14, next_of_kin_id_number = $15,
            next_of_kin_address = $16
        where id = $17 and organization_id = $18
        returning {CUSTOMER_COLUMNS}
        "#
    ))
    .bind(full_name)
    .bind(clean(&input.email))
    .bind(clean(&input.phone))
    .bind(id_number)
    .bind(clean(&input.title))
    .bind(to_pg(&input.customer_type))
    .bind(clean(&input.kra_pin))
    .bind(clean(&input.postal_address))
    .bind(clean(&input.city))
    .bind(clean(&input.physical_address))
    .bind(clean(&input.legacy_customer_number))
    .bind(clean(&input.next_of_kin_name))
    .bind(clean(&input.next_of_kin_relationship))
    .bind(clean(&input.next_of_kin_mobile))
    .bind(clean(&input.next_of_kin_id_number))
    .bind(clean(&input.next_of_kin_address))
    .bind(id)
    .bind(auth.organization_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db_err)
            if db_err.constraint() == Some("customers_legacy_number_uidx") =>
        {
            AppError::conflict("That legacy customer number is already in use.")
        }
        _ => AppError::from(e),
    })?;

    Ok(Json(row.ok_or(AppError::NotFound)?.into_domain()?))
}

async fn update_lead(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateLeadInput>,
) -> Result<Json<Customer>, AppError> {
    auth.require_permission(PERM_CUSTOMERS_LEADS_UPDATE)?;

    let notes = input.notes.as_deref().map(str::trim).filter(|s| !s.is_empty());

    let row: Option<CustomerRow> = sqlx::query_as(&format!(
        r#"
        update customers set stage = $1, next_follow_up_at = $2, notes = $3
        where id = $4 and organization_id = $5
        returning {CUSTOMER_COLUMNS}
        "#
    ))
    .bind(to_pg(&input.stage))
    .bind(input.next_follow_up_at)
    .bind(notes)
    .bind(id)
    .bind(auth.organization_id)
    .fetch_optional(&state.db)
    .await?;

    Ok(Json(row.ok_or(AppError::NotFound)?.into_domain()?))
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
    let customer_row: Option<CustomerRow> = sqlx::query_as(&format!(
        r#"select {CUSTOMER_COLUMNS} from customers where id = $1 and organization_id = $2"#
    ))
    .bind(id)
    .bind(auth.organization_id)
    .fetch_optional(&state.db)
    .await?;
    let customer_row = customer_row.ok_or(AppError::NotFound)?;

    // Joined through `sale_customers` (not `ps.customer_id = $1` directly)
    // so a co-buyer sees the sale too, not just the primary buyer — see
    // `database/migrations/0026_sale_plots_and_customers.sql`.
    let sale_rows: Vec<CustomerSaleRow> = sqlx::query_as(
        r#"
        select ps.id as sale_id, pl.id as plot_id, pr.id as project_id, pl.plot_number,
            pr.name as project_name, ps.payment_mode, ps.agreed_price, pl.status as plot_status,
            pla.id as loan_account_id
        from plot_sales ps
        join sale_customers sc on sc.sale_id = ps.id
        join plots pl on pl.id = ps.plot_id
        join projects pr on pr.id = pl.project_id
        left join plot_loan_accounts pla on pla.sale_id = ps.id
        where sc.customer_id = $1
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
        customer: customer_row.into_domain()?,
        sales,
    }))
}
