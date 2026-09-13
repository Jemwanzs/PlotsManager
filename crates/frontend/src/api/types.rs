use chrono::NaiveDate;
use domain::{AreaUnit, Customer, Payment, PaymentMode, Plot, PlotLoanAccount, User};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum ApiError {
    #[error("{0}")]
    InvalidCredentials(String),
    #[error("not found")]
    NotFound,
    #[error("not signed in")]
    #[allow(dead_code)] // returned by a real 401 once api::http is wired up; nothing constructs it yet
    Unauthenticated,
    #[error("network error: {0}")]
    Network(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthSession {
    pub token: String,
    pub user: User,
}

/// A project plus the counts a list screen needs, without shipping every
/// plot over the wire just to show "12 available / 40 plots".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub id: Uuid,
    pub name: String,
    pub code: String,
    pub location: String,
    pub status: domain::ProjectStatus,
    pub total_plots: u32,
    pub available_plots: u32,
    pub sold_plots: u32,
}

/// A new land project. Deliberately narrower than the full field set in
/// docs/05 (GPS boundary, surveyor/legal info, phases, supporting
/// documents) — this is enough to register a project and start adding
/// plots to it; the rest lands with document upload/map versioning
/// (Phase 3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProjectInput {
    pub name: String,
    pub code: String,
    pub location: String,
    pub total_size: Decimal,
    pub area_unit: AreaUnit,
}

/// A new plot within a project. `plot_number` must be unique **within
/// its project** (docs/05's fix for the legacy system's global-uniqueness
/// bug — see docs/02 §3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePlotInput {
    pub project_id: Uuid,
    pub plot_number: String,
    pub size: Decimal,
    pub asking_price: Decimal,
    pub minimum_price: Decimal,
}

/// One row in a project's plot inventory, with its status color resolved
/// server-side eventually (org-configurable per docs/05) — hardcoded to
/// the suggested defaults in `api::mock` for now.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlotWithColor {
    pub plot: Plot,
    pub status_label: String,
    pub status_color: String,
}

/// A customer plus the plot count a list screen needs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerSummary {
    pub customer: Customer,
    pub plots_owned: u32,
}

/// One row in a customer's purchase history — the sale plus enough about
/// the plot/project to render without a second round trip.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerSaleView {
    pub sale_id: Uuid,
    pub plot_id: Uuid,
    pub project_id: Uuid,
    pub plot_number: String,
    pub project_name: String,
    pub payment_mode: PaymentMode,
    pub agreed_price: Decimal,
    pub status_label: String,
    pub status_color: String,
    /// Set for Lipa Pole Pole sales only — a Full Cash sale has no Plot
    /// Loan Account (see docs/08 §2.1 vs §2.2/2.3; the `payments` table
    /// itself is keyed to `loan_account_id`, not a sale, so there's
    /// nothing to link for cash sales yet).
    pub loan_account_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerDetail {
    pub customer: Customer,
    pub sales: Vec<CustomerSaleView>,
}

/// Only `full_name` is truly required — the legacy system
/// (docs/02 §6) required a full KYC set (title, ID, postal address,
/// city, KRA PIN, join date, photos) before a customer could be saved at
/// all, which is precisely why walk-in leads never made it into that
/// system until someone had time to do full data entry. Keeping this
/// deliberately minimal, matching the `domain::Customer` fields that
/// actually exist today, so a customer can be captured the moment
/// they're interested and enriched later.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCustomerInput {
    pub full_name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub id_number: Option<String>,
}

/// What it takes to reserve a plot for a customer — the first step of the
/// sales workflow (docs/07). For a Lipa Pole Pole payment mode this also
/// creates a Plot Loan Account (docs/08 §3), with a fixed 12-instalment/
/// 10%-deposit default — a real UI for choosing tenor/deposit/interest is
/// still future work, this just needs *a* schedule to exist to build the
/// payment-capture screen against.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSaleInput {
    pub plot_id: Uuid,
    pub customer_id: Uuid,
    pub payment_mode: PaymentMode,
    pub agreed_price: Decimal,
}

/// A Plot Loan Account plus enough about the plot/project/customer to
/// render its detail screen without three more round trips.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoanAccountDetail {
    pub account: PlotLoanAccount,
    pub plot_id: Uuid,
    pub plot_number: String,
    pub project_id: Uuid,
    pub project_name: String,
    pub customer_id: Uuid,
    pub customer_name: String,
    pub status_label: String,
    pub status_color: String,
    pub payments: Vec<Payment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordPaymentInput {
    pub loan_account_id: Uuid,
    pub amount: Decimal,
    pub payment_date: NaiveDate,
    pub method: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardSummary {
    pub total_customers: u32,
    pub total_projects: u32,
    pub total_plots: u32,
    pub total_sales_count: u32,
    pub total_sales_value: rust_decimal::Decimal,
    pub active_loans_count: u32,
    pub active_loan_book: rust_decimal::Decimal,
    pub performing_count: u32,
    pub performing_amount: rust_decimal::Decimal,
    pub non_performing_count: u32,
    pub non_performing_amount: rust_decimal::Decimal,
}
