//! The generic document vault — legacy-migration-readiness gap
//! analysis section 4-6: a single polymorphic table/type rather than
//! per-entity attachment columns, so any entity (customer, plot,
//! project, sale, loan account, payment) can carry an unlimited number
//! of scanned documents without a schema change per entity type.
//!
//! `file_data` (the actual bytes) is deliberately NOT part of
//! `DocumentMeta` — it's fetched via a separate `/file` route, the
//! same split `domain::ProjectMapSummary` uses for its image, so a
//! page that only needs "what documents exist" never pulls multi-MB
//! payloads it isn't displaying yet.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentEntityType {
    Customer,
    Plot,
    Project,
    Sale,
    LoanAccount,
    Payment,
    TitleRecord,
}

impl DocumentEntityType {
    pub fn as_str(&self) -> &'static str {
        match self {
            DocumentEntityType::Customer => "customer",
            DocumentEntityType::Plot => "plot",
            DocumentEntityType::Project => "project",
            DocumentEntityType::Sale => "sale",
            DocumentEntityType::LoanAccount => "loan_account",
            DocumentEntityType::Payment => "payment",
            DocumentEntityType::TitleRecord => "title_record",
        }
    }
}

/// Document metadata as returned to the frontend — never carries the
/// bytes themselves (see module docs).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMeta {
    pub id: Uuid,
    pub entity_type: DocumentEntityType,
    pub entity_id: Uuid,
    pub document_type: String,
    pub document_number: Option<String>,
    pub original_filename: String,
    pub mime_type: String,
    pub file_size: i64,
    pub issue_date: Option<NaiveDate>,
    pub expiry_date: Option<NaiveDate>,
    pub description: Option<String>,
    pub uploaded_by_name: String,
    pub uploaded_at: DateTime<Utc>,
    pub legacy_source_path: Option<String>,
}

/// The metadata half of a document upload — the file itself travels
/// alongside as a separate multipart field, never through this type
/// (see `crates/frontend/src/api/http.rs::upload_document`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadDocumentInput {
    pub entity_type: DocumentEntityType,
    pub entity_id: Uuid,
    pub document_type: String,
    pub document_number: Option<String>,
    pub issue_date: Option<NaiveDate>,
    pub expiry_date: Option<NaiveDate>,
    pub description: Option<String>,
}
