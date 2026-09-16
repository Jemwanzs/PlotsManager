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
    /// Side lengths (`database/migrations/0011_plot_dimensions.sql`) —
    /// a separate, user-facing attribute from `size`, never derived from
    /// or overwriting it. `None` means not recorded yet, not zero.
    pub side_1: Option<Decimal>,
    pub side_2: Option<Decimal>,
    /// Only meaningful when both sides are `Some`. Stored per plot
    /// (rather than assumed app-wide) so a future org-level unit
    /// preference can vary it without another migration — every plot
    /// today is created with `"ft"`, the only unit the UI offers.
    pub dimension_unit: String,
    pub asking_price: Decimal,
    pub minimum_price: Decimal,
    pub status: PlotStatus,
    /// Unused since the interactive-map v1 slice (see `ProjectMap`'s
    /// module docs) links polygons to plots via `MapFeature.plot_id`
    /// directly instead — kept only so existing rows/queries don't
    /// need a migration to drop the column.
    pub map_feature_id: Option<String>,
    pub assigned_customer_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

/// "80 × 100 ft", or `None` when either side hasn't been recorded —
/// callers show "Not specified" for that case rather than a misleading
/// "0 × 0". Shared (not duplicated per call site) so the plot grid, the
/// plot detail panel, the map's selected-plot view, and any future
/// report all render dimensions identically.
pub fn format_dimensions(side_1: Option<Decimal>, side_2: Option<Decimal>, unit: &str) -> Option<String> {
    match (side_1, side_2) {
        (Some(a), Some(b)) => Some(format!("{a} \u{d7} {b} {unit}")),
        _ => None,
    }
}
