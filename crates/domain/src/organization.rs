use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A tenant. Every other record in the system is scoped to one organisation;
/// no query should ever cross this boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Organization {
    pub id: Uuid,
    pub name: String,
    pub code: String,
    pub currency: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Branch {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub code: String,
    pub region: Option<String>,
    pub location: Option<String>,
    pub contact_name: Option<String>,
    pub contact_phone: Option<String>,
    pub manager_id: Option<Uuid>,
    pub manager_name: Option<String>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBranchInput {
    pub name: String,
    pub code: String,
    pub region: Option<String>,
    pub location: Option<String>,
    pub contact_name: Option<String>,
    pub contact_phone: Option<String>,
    pub manager_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateBranchInput {
    pub name: String,
    pub code: String,
    pub region: Option<String>,
    pub location: Option<String>,
    pub contact_name: Option<String>,
    pub contact_phone: Option<String>,
    pub manager_id: Option<Uuid>,
}

/// Which record types the auto-numbering engine currently issues numbers
/// for. `numbering_sequences.entity_type` (database/migrations/0010) stores
/// this as free text rather than a DB enum specifically so a later record
/// type can opt in without a migration -- this is the application-level
/// allowlist for what exists *today*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NumberingEntityType {
    Plot,
    Project,
}

impl NumberingEntityType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Plot => "plot",
            Self::Project => "project",
        }
    }
}

/// An organization's auto-numbering configuration for one entity type,
/// plus a live `preview` of the number `next_number` would format to --
/// computed with `format_sequence_number` below so the admin sees exactly
/// what the *next* generated number will look like without consuming it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NumberingConfig {
    pub entity_type: NumberingEntityType,
    pub prefix: String,
    pub include_year: bool,
    /// Plot numbering only: interpolate the owning project's `code`
    /// between the prefix/year and the padded number (e.g.
    /// `PLT-KILIMANI-0001`). Ignored for project numbering, which has no
    /// parent record to draw a code from.
    pub include_entity_code: bool,
    pub padding: u32,
    pub next_number: u32,
    pub preview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NumberingConfigInput {
    pub prefix: String,
    pub include_year: bool,
    pub include_entity_code: bool,
    pub padding: u32,
    pub next_number: u32,
}

/// Whether a manual interest/penalty charge is computed as a percentage
/// of the account's outstanding principal, or a flat amount every time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RateType {
    Percentage,
    Fixed,
}

/// One side (interest or penalty) of `FinancePolicy` — whether it's
/// switched on at all, and if so, how a manual charge's suggested
/// amount is computed. This doesn't *apply* anything on its own (no
/// scheduler exists to charge automatically — see `routes/loan_accounts
/// .rs::post_charge`'s own docs); it only feeds the "use policy rate"
/// suggestion on that manual charge form, and future automatic
/// charging can read the exact same field once that phase exists.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ChargePolicy {
    pub enabled: bool,
    pub rate_type: RateType,
    pub rate_value: Decimal,
}

/// The three previously-hardcoded finance behaviors this organization
/// can now actually configure: which order a payment clears penalty/
/// interest/principal in (`routes/loan_accounts.rs::allocate_waterfall`
/// used to hardcode penalty -> interest -> principal), how many days
/// past a due date before a schedule instalment counts as overdue
/// (`0022_repayment_schedule.sql`'s views used to hardcode 7), and
/// each charge type's suggested manual-charge rate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinancePolicy {
    /// A permutation of exactly `["penalty", "interest", "principal"]`
    /// — the order `allocate_waterfall` applies a payment in.
    pub allocation_order: Vec<String>,
    pub grace_period_days: i32,
    pub interest: ChargePolicy,
    pub penalty: ChargePolicy,
}

/// `GET /api/v1/settings` — the "Organization / System Configuration"
/// screen's whole payload in one round trip.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationSettings {
    pub organization_id: Uuid,
    pub name: String,
    pub currency: String,
    pub date_format: String,
    pub timezone: String,
    pub plot_numbering: NumberingConfig,
    pub project_numbering: NumberingConfig,
    pub finance_policy: FinancePolicy,
    /// The default agent-commission rate (percent of `agreed_price`),
    /// applied to every sale unless the project it's on has its own
    /// `Project::commission_rate_percent` override — accrual tracking
    /// only, computed once at sale creation
    /// (`routes/sales.rs::execute_sale`), not a payout workflow.
    pub default_commission_rate_percent: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateOrganizationSettingsInput {
    pub currency: String,
    pub date_format: String,
    pub timezone: String,
    pub plot_numbering: NumberingConfigInput,
    pub project_numbering: NumberingConfigInput,
    pub finance_policy: FinancePolicy,
    pub default_commission_rate_percent: Decimal,
}

/// `POST /api/v1/settings/numbering/:entity_type/next` — the number that
/// was just atomically issued (the counter is already incremented by the
/// time this comes back), for pre-filling a plot/project creation form's
/// number field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedNumber {
    pub number: String,
}

/// Shared by the backend (computing `NumberingConfig.preview`, and the
/// real generated number on `POST /settings/numbering/:entity_type/next`)
/// and the frontend (a live preview while the admin edits an as-yet-
/// unsaved config on the Settings page, with no round trip per
/// keystroke) — one algorithm, one place, per this crate's whole reason
/// for existing (see lib.rs module docs).
pub fn format_sequence_number(
    prefix: &str,
    include_year: bool,
    entity_code: Option<&str>,
    padding: u32,
    number: u32,
) -> String {
    let mut parts: Vec<String> = Vec::new();
    let prefix = prefix.trim();
    if !prefix.is_empty() {
        parts.push(prefix.to_string());
    }
    if include_year {
        parts.push(Utc::now().format("%Y").to_string());
    }
    if let Some(code) = entity_code.map(str::trim).filter(|c| !c.is_empty()) {
        parts.push(code.to_string());
    }
    parts.push(format!("{:0width$}", number, width = padding.max(1) as usize));
    parts.join("-")
}
