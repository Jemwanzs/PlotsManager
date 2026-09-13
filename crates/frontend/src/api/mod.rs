//! API-first frontend design: every component reaches data through
//! `ApiClient`, never through `mock`/`http` directly. See
//! docs/12-api-and-integration-design.md and the module docs on `mock`.

mod http;
mod mock;
mod plot_status;
mod types;

use uuid::Uuid;

pub use plot_status::status_meta;
pub use types::*;

#[derive(Clone)]
pub enum ApiClient {
    Mock(mock::MockApi),
    Http(http::HttpApi),
}

impl ApiClient {
    pub fn new_mock() -> Self {
        Self::Mock(mock::MockApi::new())
    }

    #[allow(dead_code)] // wired in once crates/backend exposes real routes
    pub fn new_http(base_url: impl Into<String>) -> Self {
        Self::Http(http::HttpApi::new(base_url))
    }

    pub async fn login(&self, email: &str, password: &str) -> Result<AuthSession, ApiError> {
        match self {
            Self::Mock(api) => api.login(email, password).await,
            Self::Http(api) => api.login(email, password).await,
        }
    }

    pub async fn dashboard_summary(&self) -> Result<DashboardSummary, ApiError> {
        match self {
            Self::Mock(api) => api.dashboard_summary().await,
            Self::Http(api) => api.dashboard_summary().await,
        }
    }

    pub async fn list_projects(&self) -> Result<Vec<ProjectSummary>, ApiError> {
        match self {
            Self::Mock(api) => api.list_projects().await,
            Self::Http(api) => api.list_projects().await,
        }
    }

    pub async fn get_project(&self, id: Uuid) -> Result<domain::Project, ApiError> {
        match self {
            Self::Mock(api) => api.get_project(id).await,
            Self::Http(api) => api.get_project(id).await,
        }
    }

    pub async fn list_plots(&self, project_id: Uuid) -> Result<Vec<PlotWithColor>, ApiError> {
        match self {
            Self::Mock(api) => api.list_plots(project_id).await,
            Self::Http(api) => api.list_plots(project_id).await,
        }
    }

    pub async fn list_customers(&self) -> Result<Vec<CustomerSummary>, ApiError> {
        match self {
            Self::Mock(api) => api.list_customers().await,
            Self::Http(api) => api.list_customers().await,
        }
    }

    pub async fn get_customer(&self, id: Uuid) -> Result<CustomerDetail, ApiError> {
        match self {
            Self::Mock(api) => api.get_customer(id).await,
            Self::Http(api) => api.get_customer(id).await,
        }
    }

    pub async fn create_sale(&self, input: CreateSaleInput) -> Result<domain::PlotSale, ApiError> {
        match self {
            Self::Mock(api) => api.create_sale(input).await,
            Self::Http(api) => api.create_sale(input).await,
        }
    }
}
