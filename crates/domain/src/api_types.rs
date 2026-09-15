//! The actual wire contract between `frontend` and `backend` — request
//! inputs and response shapes for every endpoint. Living here (not
//! duplicated in each crate) is the whole point of a shared `domain`
//! crate: `frontend::api::mock`, `frontend::api::http`, and every
//! `crates/backend` route handler are all describing the same operations,
//! so they share the same types rather than three hand-kept-in-sync
//! copies. See docs/12-api-and-integration-design.md.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    ApprovalRequest, AreaUnit, Customer, LeadStage, MapPolygons, Payment, PaymentMode, Plot,
    PlotLoanAccount, ProjectStatus, Quotation, User,
};

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApiError {
    #[error("{0}")]
    InvalidCredentials(String),
    #[error("not found")]
    NotFound,
    #[error("not signed in")]
    Unauthenticated,
    #[error("network error: {0}")]
    Network(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthSession {
    pub token: String,
    pub user: User,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginInput {
    pub email: String,
    pub password: String,
}

/// Creates a brand-new tenant: the organization, its first (admin) user,
/// and a 48-hour trial subscription, all in one transaction — see
/// `crates/backend/src/routes/auth.rs`'s `signup` handler and
/// docs/16-billing-and-subscriptions.md. Returns an `AuthSession` just
/// like login, since signing up should land you straight in the app.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignupInput {
    pub organization_name: String,
    pub organization_code: String,
    pub admin_full_name: String,
    pub admin_email: String,
    pub admin_password: String,
}

/// A project plus the counts a list screen needs, without shipping every
/// plot over the wire just to show "12 available / 40 plots".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub id: Uuid,
    pub name: String,
    pub code: String,
    pub location: String,
    pub status: ProjectStatus,
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
/// server-side (org-configurable per docs/05) — hardcoded to the
/// suggested defaults for now (`domain::plot_status_meta`).
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
/// deliberately minimal, matching the `Customer` fields that actually
/// exist today, so a customer can be captured the moment they're
/// interested and enriched later.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCustomerInput {
    pub full_name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub id_number: Option<String>,
    /// How this lead found us — free text (e.g. "Referral", "Walk-in",
    /// "Website"), not a fixed list: the legacy system and every spec
    /// doc are silent on a standard set, so this isn't a constraint to
    /// invent one.
    pub source: Option<String>,
}

/// Moving a lead through the pipeline (`docs/07`'s "Sales funnel") — a
/// separate input from `CreateCustomerInput` because updating stage/
/// notes/follow-up happens repeatedly over a lead's life, independently
/// of (and usually much more often than) editing their contact details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateLeadInput {
    pub stage: LeadStage,
    pub next_follow_up_at: Option<NaiveDate>,
    pub notes: Option<String>,
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
    pub total_sales_value: Decimal,
    pub active_loans_count: u32,
    pub active_loan_book: Decimal,
    pub performing_count: u32,
    pub performing_amount: Decimal,
    pub non_performing_count: u32,
    pub non_performing_amount: Decimal,
}

/// Cross-tenant administration — `crates/backend/src/routes/platform.rs`,
/// gated by `AuthUser.is_platform_owner`. See
/// `database/migrations/0004_platform_ownership.sql`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformOrganizationSummary {
    pub id: Uuid,
    pub name: String,
    pub code: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub user_count: i64,
    pub subscription_status: Option<String>,
    pub trial_ends_at: Option<DateTime<Utc>>,
    pub plan_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformOrganizationUser {
    pub id: Uuid,
    pub full_name: String,
    pub email: String,
    pub is_active: bool,
    pub is_platform_owner: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformAccessLogEntry {
    pub actor_id: Option<Uuid>,
    pub actor_name: Option<String>,
    pub action: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformOrganizationDetail {
    #[serde(flatten)]
    pub summary: PlatformOrganizationSummary,
    pub users: Vec<PlatformOrganizationUser>,
    pub recent_access: Vec<PlatformAccessLogEntry>,
}

/// Creates a `Quotation` in `Draft` status — see
/// `crates/backend/src/routes/quotations.rs`. `below_minimum_price` on
/// the response types is still purely informational *here*, at draft
/// creation — a draft is just an offer being drafted, nothing is
/// committed yet. The gate is enforced later, when that offer would
/// become a real sale (`POST /quotations/:id/accept`, and the
/// equivalent direct path `POST /sales`) — see
/// `crates/backend/src/routes/approvals.rs::gate_price`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateQuotationInput {
    pub plot_id: Uuid,
    pub customer_id: Uuid,
    pub payment_mode: PaymentMode,
    pub quoted_price: Decimal,
    pub valid_until: NaiveDate,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotationSummary {
    pub quotation: Quotation,
    pub plot_number: String,
    pub project_name: String,
    pub customer_name: String,
    pub status_label: String,
    pub status_color: String,
    pub is_expired: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotationDetail {
    pub quotation: Quotation,
    pub plot_id: Uuid,
    pub plot_number: String,
    pub project_id: Uuid,
    pub project_name: String,
    pub asking_price: Decimal,
    pub minimum_price: Decimal,
    pub customer_id: Uuid,
    pub customer_name: String,
    pub status_label: String,
    pub status_color: String,
    pub is_expired: bool,
    pub below_minimum_price: bool,
}

/// `ApprovalRequest` plus the display fields its list/detail views need
/// — same shape as `QuotationSummary` above.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequestSummary {
    pub request: ApprovalRequest,
    pub plot_number: String,
    pub project_name: String,
    pub customer_name: String,
    pub requested_by_name: String,
    pub decided_by_name: Option<String>,
    pub status_label: String,
    pub status_color: String,
}

/// Body for `POST /approvals/:id/approve` and `.../reject` — a note is
/// optional either way (approving a below-minimum price is often
/// self-explanatory; rejecting usually isn't, but nothing here forces
/// the caller to explain).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DecideApprovalInput {
    pub notes: Option<String>,
}

/// Metadata for `GET /projects/:id/map` — no image bytes (those come
/// from the separate `GET /projects/:id/map/image` route, so a page
/// that only needs "does a map exist / what are the polygons" never
/// pulls a multi-MB payload for it).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMapSummary {
    pub exists: bool,
    pub image_content_type: Option<String>,
    pub polygons: MapPolygons,
    pub updated_at: Option<DateTime<Utc>>,
}

/// Body for `PUT /projects/:id/map/polygons` — the client always sends
/// the full desired polygon set, not a diff; matches this codebase's
/// "derive, don't store incrementally" preference and keeps the
/// endpoint's semantics obvious.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMapPolygonsInput {
    pub polygons: MapPolygons,
}

/// One failed row from a bulk-import endpoint (`POST
/// /projects/:id/plots/bulk`, `POST /customers/bulk`) — `row` is
/// 1-based against the uploaded CSV (header excluded), matching how a
/// spreadsheet user thinks about "row 3", not a 0-based array index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkImportRowError {
    pub row: u32,
    pub message: String,
}

/// Best-effort, not all-or-nothing: onboarding data is rarely clean
/// (duplicate plot numbers, a blank required field), and failing the
/// whole batch over one bad row would be worse than importing what's
/// valid and reporting the rest — each row is validated and inserted
/// independently, exactly like `POST /projects/:id/plots` or `POST
/// /customers` would for a single row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkImportResult {
    pub created: u32,
    pub errors: Vec<BulkImportRowError>,
}
