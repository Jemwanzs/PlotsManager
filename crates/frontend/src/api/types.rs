use domain::{Plot, User};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum ApiError {
    #[error("{0}")]
    InvalidCredentials(String),
    #[error("not found")]
    NotFound,
    #[error("not signed in")]
    #[allow(dead_code)] // returned by a real 401 once api::http is wired up; nothing constructs it yet
    Unauthenticated,
    #[error("network error: {0}")]
    Network(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthSession {
    pub token: String,
    pub user: User,
}

/// A project plus the counts a list screen needs, without shipping every
/// plot over the wire just to show "12 available / 40 plots".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub id: Uuid,
    pub name: String,
    pub code: String,
    pub location: String,
    pub status: domain::ProjectStatus,
    pub total_plots: u32,
    pub available_plots: u32,
    pub sold_plots: u32,
}

/// One row in a project's plot inventory, with its status color resolved
/// server-side eventually (org-configurable per docs/05) — hardcoded to
/// the suggested defaults in `api::mock` for now.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlotWithColor {
    pub plot: Plot,
    pub status_label: String,
    pub status_color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardSummary {
    pub total_customers: u32,
    pub total_projects: u32,
    pub total_plots: u32,
    pub total_sales_count: u32,
    pub total_sales_value: rust_decimal::Decimal,
    pub active_loans_count: u32,
    pub active_loan_book: rust_decimal::Decimal,
    pub performing_count: u32,
    pub performing_amount: rust_decimal::Decimal,
    pub non_performing_count: u32,
    pub non_performing_amount: rust_decimal::Decimal,
}
