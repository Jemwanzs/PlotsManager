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
}
