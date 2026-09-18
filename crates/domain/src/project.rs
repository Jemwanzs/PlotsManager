use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AreaUnit {
    Hectares,
    Acres,
    SquareMetres,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectStatus {
    Planning,
    Active,
    OnHold,
    SoldOut,
    Closed,
}

/// A land project: a parcel subdivided into plots for sale.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub branch_id: Option<Uuid>,
    pub name: String,
    pub code: String,
    pub location: String,
    pub original_title_number: Option<String>,
    pub total_size: Decimal,
    pub area_unit: AreaUnit,
    pub status: ProjectStatus,
    pub assigned_manager_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

/// One image plus one polygon set per project — the minimal v1 slice
/// of docs/06-interactive-map-engine.md's Phase 3, deliberately
/// without its draft/pending-approval/published versioning workflow
/// (see `database/migrations/0009_project_map.sql`'s module comment).
/// Re-uploading the image replaces this outright.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMap {
    pub project_id: Uuid,
    pub image_content_type: String,
    pub polygons: MapPolygons,
    pub uploaded_by: Uuid,
    pub updated_at: DateTime<Utc>,
}

/// Plot boundaries in *pixel* space against the uploaded image, not
/// geographic coordinates — v1's tech choice is a plain image with an
/// SVG overlay (docs/06:51-66), not a real GIS layer.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MapPolygons {
    pub image_width: f64,
    pub image_height: f64,
    pub features: Vec<MapFeature>,
}

/// A drawn shape on the map — the visual representation of a plot, not
/// the plot record itself (`crate::Plot` remains the source of truth
/// for size/price/status/etc; a feature only carries what's needed to
/// render and locate it). `plot_id` starts `None`: a freshly-drawn
/// shape is a *draft*, unlinked to any inventory record, until
/// "Create Plot" or "Link Existing Plot" resolves it
/// (`crates/backend/src/routes/project_map.rs`). `label` is the
/// original source-plan label ("Lot 1", "Plot A", "Block B/12") —
/// deliberately separate from `Plot::plot_number` (the platform's own
/// configurable numbering, `domain::organization::format_sequence_number`)
/// since neither should overwrite the other.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapFeature {
    pub id: String,
    pub plot_id: Option<Uuid>,
    pub label: Option<String>,
    pub points: Vec<[f64; 2]>,
}

/// `POST /api/v1/projects/:id/map/features/:feature_id/link-plot` —
/// attaches an already-existing, not-yet-mapped plot to a drawn shape.
/// The sibling "Create Plot" action instead takes a full
/// `CreatePlotInput`, reusing plot creation itself rather than
/// introducing a second plot-registration path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkPlotInput {
    pub plot_id: Uuid,
}
