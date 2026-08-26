use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Configurable plot lifecycle. Labels/colours are tenant-configurable in
/// the database (`plot_status_config` table); this enum is the fixed set of
/// underlying states the workflow engine understands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlotStatus {
    Available,
    Selected,
    TemporarilyHeld,
    Reserved,
    Booked,
    UnderApproval,
    Sold,
    TransferInProgress,
    Transferred,
    Blocked,
    Disputed,
    Cancelled,
}

/// A single plot: the unit of inventory and sale.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plot {
    pub id: Uuid,
    pub project_id: Uuid,
    pub plot_number: String,
    pub title_number: Option<String>,
    pub size: Decimal,
    pub asking_price: Decimal,
    pub minimum_price: Decimal,
    pub status: PlotStatus,
    /// Index into the published ProjectMapVersion's polygon FeatureCollection.
    pub map_feature_id: Option<String>,
    pub assigned_customer_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}
