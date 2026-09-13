//! Real-backend implementation. Establishes the contract `api::mock`
//! already implements against, but the routes don't exist on the backend
//! yet (see docs/14-development-roadmap.md) — every method here is
//! genuinely unimplemented, not a guess at a wire format. Wiring this up
//! is: implement the matching Axum route in `crates/backend`, then fill
//! in the `gloo_net` call here. No UI component changes when that happens.

use uuid::Uuid;

use super::types::{
    ApiError, AuthSession, CreateCustomerInput, CreateSaleInput, CustomerDetail, CustomerSummary,
    DashboardSummary, LoanAccountDetail, PlotWithColor, ProjectSummary, RecordPaymentInput,
};

#[derive(Clone)]
pub struct HttpApi {
    #[allow(dead_code)]
    base_url: String,
}

impl HttpApi {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
        }
    }

    pub async fn login(&self, _email: &str, _password: &str) -> Result<AuthSession, ApiError> {
        Err(ApiError::Network(
            "backend auth endpoints aren't built yet".to_string(),
        ))
    }

    pub async fn dashboard_summary(&self) -> Result<DashboardSummary, ApiError> {
        Err(ApiError::Network(
            "backend dashboard endpoint isn't built yet".to_string(),
        ))
    }

    pub async fn list_projects(&self) -> Result<Vec<ProjectSummary>, ApiError> {
        Err(ApiError::Network(
            "backend projects endpoint isn't built yet".to_string(),
        ))
    }

    pub async fn get_project(&self, _id: Uuid) -> Result<domain::Project, ApiError> {
        Err(ApiError::Network(
            "backend projects endpoint isn't built yet".to_string(),
        ))
    }

    pub async fn list_plots(&self, _project_id: Uuid) -> Result<Vec<PlotWithColor>, ApiError> {
        Err(ApiError::Network(
            "backend plots endpoint isn't built yet".to_string(),
        ))
    }

    pub async fn list_customers(&self) -> Result<Vec<CustomerSummary>, ApiError> {
        Err(ApiError::Network(
            "backend customers endpoint isn't built yet".to_string(),
        ))
    }

    pub async fn get_customer(&self, _id: Uuid) -> Result<CustomerDetail, ApiError> {
        Err(ApiError::Network(
            "backend customers endpoint isn't built yet".to_string(),
        ))
    }

    pub async fn create_customer(&self, _input: CreateCustomerInput) -> Result<domain::Customer, ApiError> {
        Err(ApiError::Network(
            "backend customers endpoint isn't built yet".to_string(),
        ))
    }

    pub async fn create_sale(&self, _input: CreateSaleInput) -> Result<domain::PlotSale, ApiError> {
        Err(ApiError::Network(
            "backend sales endpoint isn't built yet".to_string(),
        ))
    }

    pub async fn get_loan_account(&self, _id: Uuid) -> Result<LoanAccountDetail, ApiError> {
        Err(ApiError::Network(
            "backend loan account endpoint isn't built yet".to_string(),
        ))
    }

    pub async fn record_payment(&self, _input: RecordPaymentInput) -> Result<domain::Payment, ApiError> {
        Err(ApiError::Network(
            "backend payments endpoint isn't built yet".to_string(),
        ))
    }
}
