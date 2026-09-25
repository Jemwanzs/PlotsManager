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
    PlotLoanAccount, PlotStatusCount, ProjectStatus, Quotation, SaleLifecycleStatus, User,
};

/// The full commercial position of one plot — what shows in Plot
/// Details when clicked from the grid, the map, or search
/// (`GET /api/v1/projects/:id/plots/:plot_id/commercial-summary`,
/// `crates/backend/src/routes/projects.rs`). Deliberately not part of
/// `PlotWithColor`/the plots-list response: the list renders a whole
/// project's plots at once (mostly `Available`, no sale to join), so
/// this join only runs for the one plot actually opened. `sale` is
/// `None` for an untouched plot — no reservation, no buyer, nothing
/// else on this type is meaningful.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlotCommercialSummary {
    pub plot: Plot,
    pub status_label: String,
    pub status_color: String,
    pub sale: Option<PlotSaleSummary>,
}

/// A sale/agreement's ("legacy data migration readiness" gap analysis)
/// customer, beyond the single "primary" one `plot_sales.customer_id`
/// still points at. `role` distinguishes a real joint buyer from a
/// company representative — both are "on the sale" but mean different
/// things legally. See `database/migrations/0026_sale_plots_and_
/// customers.sql`'s own docs for why this is additive alongside the
/// existing primary column rather than replacing it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SaleCustomerRole {
    Primary,
    Joint,
    Representative,
}

/// One additional (non-primary) buyer to attach when creating a sale —
/// `CreateSaleInput::additional_customers`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdditionalSaleCustomer {
    pub customer_id: Uuid,
    pub role: SaleCustomerRole,
}

/// A co-buyer as returned on a sale's read side (`PlotSaleSummary::
/// co_buyers`) — enough to display and link to them without a further
/// round trip.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaleCustomerRef {
    pub customer_id: Uuid,
    pub customer_name: String,
    pub role: SaleCustomerRole,
}

/// An additional (non-primary) plot on a sale (`PlotSaleSummary::
/// additional_plots`) — the legacy loan registry's "one loan across
/// several plots" pattern (e.g. `PL.7,8,9,10` under one loan number).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalePlotRef {
    pub plot_id: Uuid,
    pub plot_number: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlotSaleSummary {
    pub sale_id: Uuid,
    pub customer_id: Uuid,
    pub customer_name: String,
    pub payment_mode: PaymentMode,
    pub agreed_price: Decimal,
    pub created_at: DateTime<Utc>,
    /// Whether this sale is still live, or was cancelled/repossessed —
    /// see `database/migrations/0030_sale_lifecycle.sql`. A cancelled/
    /// repossessed sale's details still show here (historical record),
    /// distinguished by this field and the two below rather than by
    /// disappearing once terminated.
    pub lifecycle_status: SaleLifecycleStatus,
    pub status_reason: Option<String>,
    pub status_changed_at: Option<DateTime<Utc>>,
    /// `None` for a full-cash sale — no loan account exists for one,
    /// it's paid in full the moment it's recorded (see
    /// `routes/sales.rs::execute_sale`'s own docs on why). Carries
    /// `PlotLoanAccount` wholesale rather than duplicating its fields
    /// here — Deposit Required/Paid, Amount Paid, Outstanding Balance,
    /// Interest Rate, and Days in Arrears are all already on it.
    pub loan_account: Option<PlotLoanAccount>,
    pub loan_status_label: Option<String>,
    pub loan_status_color: Option<String>,
    /// Buyers on this sale beyond the primary one above — empty for
    /// the overwhelming majority of sales.
    pub co_buyers: Vec<SaleCustomerRef>,
    /// Other plots this same sale/loan also covers, beyond the one
    /// this summary was requested for — empty for the overwhelming
    /// majority of sales.
    pub additional_plots: Vec<SalePlotRef>,
}

/// One row of the organization-wide Finance → Loan Accounts list
/// (`GET /api/v1/finance/loan-accounts`) — every Lipa Pole Pole
/// receivable across every project, which nothing surfaced as a single
/// list before this (a loan account was only reachable by drilling into
/// the customer that holds it). Same shape as `LoanAccountDetail` minus
/// `payments`, which this list view has no use for.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoanAccountSummary {
    pub account: PlotLoanAccount,
    pub plot_id: Uuid,
    pub plot_number: String,
    pub project_id: Uuid,
    pub project_name: String,
    pub customer_id: Uuid,
    pub customer_name: String,
    pub status_label: String,
    pub status_color: String,
}

/// `GET /api/v1/finance/receivables-breakdown` — org-wide interest and
/// penalty outstanding, summed straight from `loan_ledger_entries`
/// (the same source `outstanding_components` reads per-account in
/// `routes/loan_accounts.rs`, just aggregated across every account).
/// Principal outstanding isn't included here: it's `total_outstanding
/// - interest_outstanding - penalty_outstanding`, and the caller
/// already has `total_outstanding` from summing the loan-accounts list.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FinanceReceivablesBreakdown {
    pub interest_outstanding: Decimal,
    pub penalty_outstanding: Decimal,
}

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

/// Creates a brand-new tenant application: the organization (in
/// `pending_approval`), its first (admin) user, and a recorded Terms &
/// Conditions acceptance, all in one transaction — see
/// `crates/backend/src/routes/auth.rs`'s `signup` handler. Deliberately
/// does **not** create a trial subscription or issue a session token:
/// per the tenant-onboarding spec, sign-up only creates the
/// application — the trial starts when the Platform Owner approves it
/// (`routes/platform.rs::approve_organization`), so an applicant
/// waiting for review never loses trial days to that wait. Returns
/// `SignupResult`, not `AuthSession` — nobody is signed in yet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignupInput {
    pub organization_name: String,
    pub organization_code: String,
    pub admin_full_name: String,
    pub admin_email: String,
    pub admin_password: String,
    pub admin_mobile: String,
    pub business_registration_number: Option<String>,
    pub sector: String,
    pub business_location: String,
    pub contact_person_name: String,
    pub expected_users: Option<i32>,
    pub number_of_branches: Option<i32>,
    pub preferred_package_code: Option<String>,
    /// Which `TermsVersion` the applicant was actually shown — the
    /// backend rejects the submission if this isn't still the current
    /// version (it changed under them mid-fill) rather than silently
    /// recording acceptance of a version they never saw.
    pub terms_version_id: Uuid,
    pub terms_accepted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignupResult {
    pub organization_id: Uuid,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TermsVersion {
    pub id: Uuid,
    pub version_label: String,
    pub body: String,
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
/// bug — see docs/02 §3). `side_1`/`side_2` are the plot's side lengths
/// in feet (e.g. "80 by 100") — optional, and never used to derive
/// `size`: the two describe the same plot two different ways and are
/// captured independently (see `domain::plot::format_dimensions`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePlotInput {
    pub project_id: Uuid,
    pub plot_number: String,
    pub size: Decimal,
    pub side_1: Option<Decimal>,
    pub side_2: Option<Decimal>,
    pub asking_price: Decimal,
    pub minimum_price: Decimal,
}

/// `PUT /api/v1/projects/:project_id/plots/:plot_id` — the same editable
/// fields `CreatePlotInput` takes at creation, now editable afterward
/// (docs' "Edit Plot" requirement). `project_id`/`plot_id` ride in the
/// URL, not the body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePlotInput {
    pub plot_number: String,
    pub size: Decimal,
    pub side_1: Option<Decimal>,
    pub side_2: Option<Decimal>,
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
    /// Other plots this same sale/loan also covers — legacy data
    /// migration readiness (e.g. one loan across `PL.7,8,9,10`). Empty
    /// for the overwhelming majority of sales; `#[serde(default)]` so
    /// every existing caller that doesn't know about this keeps
    /// compiling and working unchanged.
    #[serde(default)]
    pub additional_plot_ids: Vec<Uuid>,
    /// Other buyers on this sale beyond `customer_id` — real joint
    /// buyers, not a name concatenated into one `Customer` row. Empty
    /// for the overwhelming majority of sales.
    #[serde(default)]
    pub additional_customers: Vec<AdditionalSaleCustomer>,
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
    /// Net outstanding for each component, summed from the ledger the
    /// same way `outstanding_components`/`allocate_waterfall`
    /// (`routes/loan_accounts.rs`) do — lets the frontend derive
    /// `principal_outstanding` (`outstanding_balance` minus these two)
    /// for the "use policy rate" suggestion on a manual charge, without
    /// a second round trip.
    pub interest_outstanding: Decimal,
    pub penalty_outstanding: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordPaymentInput {
    pub loan_account_id: Uuid,
    pub amount: Decimal,
    pub payment_date: NaiveDate,
    pub method: String,
}

/// `POST /api/v1/sales/:id/cancel` — an administrative/mutual
/// cancellation, no loan account required. Ends the sale on every plot
/// it covers (primary and additional) and any linked loan account, but
/// leaves the outstanding balance as-is on the loan account — a
/// historical record of what was owed, not something this action
/// forgives (see `crates/backend/src/routes/sales.rs::cancel_sale`'s
/// own docs).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelSaleInput {
    pub reason: Option<String>,
}

/// `POST /api/v1/sales/:id/repossess` — default-driven, requires an
/// active (not already fully paid/closed/cancelled) loan account on
/// the sale. Distinct from `CancelSaleInput`: repossession moves every
/// plot on the sale to `Blocked` rather than `Cancelled`, signalling a
/// pending-review state rather than a clean, immediately-resellable one.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepossessSaleInput {
    pub reason: Option<String>,
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

/// `GET /api/v1/dashboard/analytics` — the Home dashboard's chart data
/// (`crates/frontend/src/pages/dashboard.rs`'s `<AnalyticsSection>`).
/// Separate endpoint from `DashboardSummary` rather than folded into
/// it: the KPI strip above needs to render immediately, and splitting
/// the heavier trend/breakdown queries into their own round trip means
/// a slow chart query never blocks the cards a director looks at first.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardAnalytics {
    pub qtd_sales_value: Decimal,
    pub qtd_sales_count: u32,
    pub ytd_sales_value: Decimal,
    pub ytd_sales_count: u32,
    /// Sales value from the same elapsed stretch of the *previous*
    /// year (Jan 1 -> today's month/day, one year back) — what YTD
    /// actually compares against; a bare "YTD total" alone doesn't say
    /// whether that's ahead or behind last year.
    pub prior_ytd_sales_value: Decimal,
    /// Trailing 12 months, oldest first, one point per month even for
    /// months with zero sales (a line chart with silently-skipped
    /// months reads as a data gap, not a real zero).
    pub monthly_trend: Vec<MonthlySalesPoint>,
    /// Current plot inventory across every project, grouped by status
    /// — the "current position" pie/donut. Uses the same
    /// `plot_status_meta` colors as the plot grid/legend elsewhere in
    /// the app, so a status means the same color everywhere.
    pub inventory_by_status: Vec<PlotStatusCount>,
    /// Year-to-date sales value per project, highest first, capped to
    /// the top 8 — which projects are actually driving this year's
    /// revenue.
    pub sales_by_project: Vec<ProjectSalesSlice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonthlySalesPoint {
    pub period_label: String,
    pub sales_value: Decimal,
    pub sales_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSalesSlice {
    pub project_name: String,
    pub sales_value: Decimal,
    pub sales_count: u32,
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
    // Onboarding-application fields, populated at sign-up
    // (`SignupInput`) and reviewed by the Platform Owner before
    // approval — see `routes/platform.rs::approve_organization`/
    // `reject_organization`.
    pub business_registration_number: Option<String>,
    pub sector: Option<String>,
    pub business_location: Option<String>,
    pub contact_person_name: Option<String>,
    pub expected_users: Option<i32>,
    pub number_of_branches: Option<i32>,
    pub preferred_package_code: Option<String>,
    pub approved_at: Option<DateTime<Utc>>,
    pub approved_by_name: Option<String>,
    pub rejected_at: Option<DateTime<Utc>>,
    pub rejected_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RejectOrganizationInput {
    pub reason: String,
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

/// One row of a bulk sales-*history* import (tenant onboarding —
/// `POST /sales/bulk`, `crates/backend/src/routes/sales.rs`).
/// Deliberately not `CreateSaleInput`: that type is for a fresh
/// reservation made today, which always starts a Lipa Pole Pole
/// account at zero paid (`execute_sale`'s 10%-deposit/12-instalment
/// default). A migrated historical sale usually isn't at zero — a
/// customer three years into their payments should import at three
/// years in, not restart — so this carries `amount_paid` instead of
/// assuming it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkSaleRow {
    pub project_code: String,
    pub plot_number: String,
    /// Matched against an existing customer's id_number, then phone,
    /// then email, in that order — the customer must already exist
    /// (import customers first).
    pub customer_lookup: String,
    pub payment_mode: PaymentMode,
    pub agreed_price: Decimal,
    pub sale_date: NaiveDate,
    /// Total already repaid toward this sale as of the import,
    /// including any deposit. Ignored for `full_cash` (paid in full
    /// by definition); 0 for a Lipa Pole Pole sale that's fully
    /// outstanding.
    pub amount_paid: Decimal,
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
