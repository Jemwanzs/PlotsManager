//! The legacy-data staging/migration framework — legacy-migration-
//! readiness gap analysis's final Phase 1 piece: Upload -> Validate ->
//! Resolve Exceptions -> Commit, with nothing landing in a production
//! table until an explicit commit, and every row's original values
//! preserved on file even if they fail validation ("never guess").
//!
//! Entity-agnostic by schema (`database/migrations/
//! 0029_migration_framework.sql`), but only `Customer` has real
//! validate/normalize/commit logic today
//! (`crates/backend/src/routes/migrations.rs`) — Plot/Project/Sale/
//! LoanAccount follow the same pattern later.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationEntityType {
    Customer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationBatchStatus {
    Staged,
    Committed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationRowStatus {
    Valid,
    Exception,
}

/// One uploaded row, exactly as parsed from the source file — column
/// name to raw cell text, no interpretation applied yet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationRawRow {
    pub source_row: u32,
    pub raw_data: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMigrationBatchInput {
    pub entity_type: MigrationEntityType,
    pub source_system: String,
    pub source_file_name: String,
    pub rows: Vec<MigrationRawRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationBatch {
    pub id: Uuid,
    pub entity_type: MigrationEntityType,
    pub source_system: String,
    pub source_file_name: String,
    pub status: MigrationBatchStatus,
    pub total_rows: i32,
    pub valid_rows: i32,
    pub exception_rows: i32,
    pub committed_rows: i32,
    pub created_by_name: String,
    pub created_at: DateTime<Utc>,
    pub committed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationStagingRow {
    pub id: Uuid,
    pub batch_id: Uuid,
    pub source_row: i32,
    pub raw_data: JsonValue,
    pub normalized_data: JsonValue,
    pub status: MigrationRowStatus,
    pub exception_message: Option<String>,
    pub committed_entity_id: Option<Uuid>,
}

/// Body of "resolve exceptions": edit a row's fields directly (same
/// shape as an uploaded row's `raw_data`) and re-validate it in place.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMigrationRowInput {
    pub fields: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationCommitResult {
    pub committed: u32,
    pub skipped_exceptions: u32,
    pub already_committed: u32,
}
