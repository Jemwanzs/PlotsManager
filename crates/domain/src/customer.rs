use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Where a `Customer` sits in the sales funnel before they buy —
/// "converted" isn't a variant here because it's derived from a
/// `plot_sales` row existing (see `database/migrations/0006_lead_pipeline.sql`),
/// not tracked as a stage that could drift out of sync with the real
/// sale. A fresh walk-in defaults to `New`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeadStage {
    New,
    Contacted,
    SiteVisit,
    Negotiating,
    Lost,
}

/// Individual, company, or a named group of co-buyers sharing one
/// customer record (e.g. "CATHERINE A OHOLA/ELIZABETH A OHOLA" in a
/// legacy register) — `joint` is a placeholder until real multi-buyer
/// sales exist as their own relationship; for now it just labels the
/// record honestly rather than forcing it into `individual`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CustomerType {
    Individual,
    Company,
    Joint,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Customer {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub full_name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub id_number: Option<String>,
    pub assigned_agent_id: Option<Uuid>,
    pub stage: LeadStage,
    pub source: Option<String>,
    pub next_follow_up_at: Option<NaiveDate>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    /// Salutation — "Mr.", "Mrs.", "Ms.", "Co." (a company), etc.
    pub title: Option<String>,
    pub customer_type: CustomerType,
    pub kra_pin: Option<String>,
    pub postal_address: Option<String>,
    pub city: Option<String>,
    pub physical_address: Option<String>,
    /// A prior system's own customer identifier (e.g. `PPP_C001`),
    /// preserved for traceability when a customer is migrated in — see
    /// `legacy_customer_number` on `database/migrations/
    /// 0025_customer_kyc_fields.sql`. Not this platform's own id
    /// (that's always `id`, a fresh UUID).
    pub legacy_customer_number: Option<String>,
    pub next_of_kin_name: Option<String>,
    pub next_of_kin_relationship: Option<String>,
    pub next_of_kin_mobile: Option<String>,
    pub next_of_kin_id_number: Option<String>,
    pub next_of_kin_address: Option<String>,
}

/// `PUT /api/v1/customers/:id` — editing a customer's own profile
/// fields, previously impossible (a customer could be created and
/// viewed, but never edited — see `PERM_CUSTOMERS_EDIT`'s doc
/// comment). Deliberately separate from `UpdateLeadInput` (stage/
/// notes/follow-up, which changes far more often) and from
/// `CreateCustomerInput` (deliberately minimal at creation time).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCustomerInput {
    pub full_name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub id_number: Option<String>,
    pub title: Option<String>,
    pub customer_type: CustomerType,
    pub kra_pin: Option<String>,
    pub postal_address: Option<String>,
    pub city: Option<String>,
    pub physical_address: Option<String>,
    pub legacy_customer_number: Option<String>,
    pub next_of_kin_name: Option<String>,
    pub next_of_kin_relationship: Option<String>,
    pub next_of_kin_mobile: Option<String>,
    pub next_of_kin_id_number: Option<String>,
    pub next_of_kin_address: Option<String>,
}

/// What kind of interaction a `CustomerActivity` records — free-text
/// summaries alone (the old single `Customer::notes` field) can't be
/// scanned at a glance the way a typed, dated history can.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityType {
    Call,
    Email,
    Sms,
    Whatsapp,
    Meeting,
    SiteVisit,
    Note,
    Other,
}

impl ActivityType {
    pub fn label(self) -> &'static str {
        match self {
            Self::Call => "Call",
            Self::Email => "Email",
            Self::Sms => "SMS",
            Self::Whatsapp => "WhatsApp",
            Self::Meeting => "Meeting",
            Self::SiteVisit => "Site Visit",
            Self::Note => "Note",
            Self::Other => "Other",
        }
    }
}

/// One entry in a customer's communications log
/// (`database/migrations/0036_customer_activities.sql`) — the "customer
/// 360" gap the roadmap flagged as genuinely missing: `Customer::notes`
/// is a single current-state field, not a history of what actually
/// happened and when. Append-only, like every other ledger/audit trail
/// in this app (`loan_ledger_entries`, `audit_log`) — a mistake gets a
/// corrective follow-up entry, not a silent edit of the record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerActivity {
    pub id: Uuid,
    pub customer_id: Uuid,
    pub activity_type: ActivityType,
    pub summary: String,
    pub occurred_at: DateTime<Utc>,
    pub created_by_name: String,
    pub created_at: DateTime<Utc>,
}

/// `POST /api/v1/customers/:id/activities`. `occurred_at` is optional —
/// logging a call as it happens needs no timestamp entry at all
/// (defaults to now), but logging one after the fact (a site visit
/// written up the next morning) needs to be dated when it actually
/// happened, not when someone got around to typing it in.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogCustomerActivityInput {
    pub activity_type: ActivityType,
    pub summary: String,
    pub occurred_at: Option<DateTime<Utc>>,
}
