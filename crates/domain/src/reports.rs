//! Report shapes — `crates/backend/src/routes/reports.rs` computes
//! these from existing `plots`/`plot_sales`/`quotations` data, no new
//! schema. docs/11-reports-and-analytics.md specifies a much larger
//! report library (collections/ageing, commissions, ...); most of it
//! depends on repayment-schedule and posted-payment infrastructure
//! that doesn't exist yet (docs/14's roadmap). These three are exactly
//! what's answerable from data already in the schema today.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{PaymentMode, PlotStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalesReportRow {
    pub sale_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub project_id: Uuid,
    pub project_name: String,
    pub plot_number: String,
    pub customer_id: Uuid,
    pub customer_name: String,
    pub agent_id: Option<Uuid>,
    pub agent_name: Option<String>,
    pub payment_mode: PaymentMode,
    pub agreed_price: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalesReport {
    pub rows: Vec<SalesReportRow>,
    pub total_count: u32,
    pub total_value: Decimal,
}

/// One (status, count, value) slice of a project's plots — `value` is
/// the sum of `asking_price` over plots in that status. Left as a list
/// rather than named performing/non-performing-style buckets (compare
/// `DashboardSummary`) so the frontend can group however's useful
/// without the backend guessing at buckets nobody's specified yet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlotStatusCount {
    pub status: PlotStatus,
    pub status_label: String,
    pub status_color: String,
    pub count: u32,
    pub value: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInventoryRow {
    pub project_id: Uuid,
    pub project_name: String,
    pub total_plots: u32,
    pub by_status: Vec<PlotStatusCount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryReport {
    pub by_project: Vec<ProjectInventoryRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPerformanceRow {
    pub agent_id: Uuid,
    pub agent_name: String,
    pub sales_count: u32,
    pub sales_value: Decimal,
    pub quotations_sent: u32,
    pub quotations_accepted: u32,
    /// Sum of `agent_commissions.commission_amount` for this agent,
    /// excluding voided rows (a cancelled/repossessed sale's
    /// commission) — accrual tracking only, not what's actually been
    /// paid out (`database/migrations/0032_agent_commissions.sql`).
    pub commission_earned: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPerformanceReport {
    pub rows: Vec<AgentPerformanceRow>,
}
