use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::PaymentMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Rejected,
}

/// A request to sell a plot below its `minimum_price`, awaiting a
/// decision from someone other than whoever requested it — see
/// `database/migrations/0008_approvals.sql` and
/// `crates/backend/src/routes/approvals.rs::gate_price` for how this
/// gates `routes/sales.rs::create_sale` and
/// `routes/quotations.rs::accept_quotation`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub plot_id: Uuid,
    pub customer_id: Uuid,
    pub agent_id: Option<Uuid>,
    pub payment_mode: PaymentMode,
    pub agreed_price: Decimal,
    pub minimum_price: Decimal,
    pub quotation_id: Option<Uuid>,
    pub requested_by: Uuid,
    pub reason: String,
    pub status: ApprovalStatus,
    pub decided_by: Option<Uuid>,
    pub decided_at: Option<DateTime<Utc>>,
    pub decision_notes: Option<String>,
    pub resulting_sale_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}
