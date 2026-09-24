//! A plot's title/ownership history — legacy-migration-readiness gap
//! analysis section 6: structured title tracking rather than the
//! single free-text `Plot::title_number` field, which can't represent
//! a title's own lifecycle (mother title -> individual title, a
//! transfer in progress, a chain of registered owners over time).
//!
//! `TitleRecord` rows are a plot's title *history*, newest first — see
//! `database/migrations/0028_title_records.sql`'s module docs for why
//! there's no backfill from `Plot::title_number` and no delete route
//! (title records are a provenance trail, never removed).

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TitleStatus {
    MotherTitle,
    IndividualTitle,
    PendingRegistration,
    Disputed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransferStatus {
    NotStarted,
    InProgress,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TitleRecord {
    pub id: Uuid,
    pub plot_id: Uuid,
    pub title_number: String,
    pub registered_owner_name: String,
    pub previous_owner_name: Option<String>,
    pub title_status: TitleStatus,
    pub transfer_status: TransferStatus,
    pub issue_date: Option<NaiveDate>,
    pub registration_date: Option<NaiveDate>,
    pub transfer_date: Option<NaiveDate>,
    pub notes: Option<String>,
    pub created_by_name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTitleRecordInput {
    pub title_number: String,
    pub registered_owner_name: String,
    pub previous_owner_name: Option<String>,
    pub title_status: TitleStatus,
    pub transfer_status: TransferStatus,
    pub issue_date: Option<NaiveDate>,
    pub registration_date: Option<NaiveDate>,
    pub transfer_date: Option<NaiveDate>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTitleRecordInput {
    pub title_number: String,
    pub registered_owner_name: String,
    pub previous_owner_name: Option<String>,
    pub title_status: TitleStatus,
    pub transfer_status: TransferStatus,
    pub issue_date: Option<NaiveDate>,
    pub registration_date: Option<NaiveDate>,
    pub transfer_date: Option<NaiveDate>,
    pub notes: Option<String>,
}
