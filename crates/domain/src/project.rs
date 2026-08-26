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

/// A versioned, published (or draft) interactive map for a project.
/// The original uploaded plan is never overwritten — every edit creates a
/// new draft version, and only an approved version is published for sales use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectMapStatus {
    Draft,
    PendingApproval,
    Approved,
    Published,
    Superseded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMapVersion {
    pub id: Uuid,
    pub project_id: Uuid,
    pub version_number: i32,
    pub status: ProjectMapStatus,
    /// Storage reference to the original uploaded plan (PDF/scan/image), immutable.
    pub source_document_url: String,
    /// GeoJSON FeatureCollection of plot polygons for this version.
    pub polygons: serde_json::Value,
    pub uploaded_by: Uuid,
    pub approved_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}
