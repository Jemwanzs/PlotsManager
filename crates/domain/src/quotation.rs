use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::PaymentMode;

/// No `Expired` variant — see `database/migrations/0007_quotations.sql`:
/// a `Sent` quotation past `valid_until` is expired, derived at read
/// time rather than stored, for the same reason `LeadStage` has no
/// `Converted` variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuotationStatus {
    Draft,
    Sent,
    Accepted,
    Rejected,
}

/// A formal price offer for a plot to a customer, ahead of a committed
/// sale (`PlotSale`). See docs/07's "quotations and offer letters" —
/// unspecified beyond that funnel-stage name, so this shape is a
/// from-scratch design, not a migrated legacy concept.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quotation {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub plot_id: Uuid,
    pub customer_id: Uuid,
    pub agent_id: Option<Uuid>,
    pub payment_mode: PaymentMode,
    pub quoted_price: Decimal,
    pub valid_until: NaiveDate,
    pub status: QuotationStatus,
    pub notes: Option<String>,
    /// Set once `accept_quotation` converts this into a real `PlotSale`
    /// — see `crates/backend/src/routes/quotations.rs`.
    pub converted_sale_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
