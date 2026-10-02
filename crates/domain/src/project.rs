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
    /// Overrides the organization's `default_commission_rate_percent`
    /// for every sale on this project — `None` means "use the org
    /// default" (see `domain::OrganizationSettings`'s doc comment).
    pub commission_rate_percent: Option<Decimal>,
    /// Why the override was set, shown alongside it in the UI — always
    /// `None` when `commission_rate_percent` is `None`.
    pub commission_rate_override_reason: Option<String>,
}

/// A project map's lifecycle state (docs/06-interactive-map-engine.md:
/// "every edit creates a new draft version; the currently approved
/// version stays locked and in force until a new one is approved").
/// At most one `Draft` and one `Published` row exist per project at a
/// time (`database/migrations/0038_project_map_versions.sql`'s partial
/// unique indexes); every prior `Published` becomes `Superseded` the
/// moment a new draft is published over it, and is never written to
/// again — permanent, read-only history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MapVersionStatus {
    Draft,
    Published,
    Superseded,
}

/// One version of a project's map — an image plus a polygon set, with
/// the version/publish metadata docs/06 calls "source-of-truth
/// controls". Replaces the v1 slice's single mutable `ProjectMap` row
/// per project (`database/migrations/0009_project_map.sql`'s module
/// comment on that deliberate cut) now that real versioning exists.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMapVersion {
    pub id: Uuid,
    pub version_number: i32,
    pub status: MapVersionStatus,
    pub image_content_type: String,
    pub polygons: MapPolygons,
    pub created_by_name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub published_by_name: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
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
