//! API-first frontend design: every component reaches data through
//! `ApiClient`, never through `mock`/`http` directly. See
//! docs/12-api-and-integration-design.md and the module docs on `mock`.

mod http;
mod mock;

use uuid::Uuid;

// The request/response types (ApiError, ProjectSummary, CreateSaleInput,
// ...) and the plot/loan status-color mappings live in `domain` — see its
// `api_types`/`status_meta` module docs — so `backend` shares the exact
// same definitions instead of a hand-kept-in-sync copy.
pub use domain::{plot_status_meta as status_meta, *};

#[derive(Clone)]
pub enum ApiClient {
    Mock(mock::MockApi),
    Http(http::HttpApi),
}

impl ApiClient {
    pub fn new_mock() -> Self {
        Self::Mock(mock::MockApi::new())
    }

    pub fn new_http(base_url: impl Into<String>) -> Self {
        Self::Http(http::HttpApi::new(base_url))
    }

    pub async fn login(&self, email: &str, password: &str) -> Result<AuthSession, ApiError> {
        match self {
            Self::Mock(api) => api.login(email, password).await,
            Self::Http(api) => api.login(email, password).await,
        }
    }

    pub async fn signup(&self, input: SignupInput) -> Result<AuthSession, ApiError> {
        match self {
            Self::Mock(api) => api.signup(input).await,
            Self::Http(api) => api.signup(input).await,
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

    pub async fn create_project(&self, input: CreateProjectInput) -> Result<domain::Project, ApiError> {
        match self {
            Self::Mock(api) => api.create_project(input).await,
            Self::Http(api) => api.create_project(input).await,
        }
    }

    pub async fn list_plots(&self, project_id: Uuid) -> Result<Vec<PlotWithColor>, ApiError> {
        match self {
            Self::Mock(api) => api.list_plots(project_id).await,
            Self::Http(api) => api.list_plots(project_id).await,
        }
    }

    pub async fn create_plot(&self, input: CreatePlotInput) -> Result<domain::Plot, ApiError> {
        match self {
            Self::Mock(api) => api.create_plot(input).await,
            Self::Http(api) => api.create_plot(input).await,
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

    pub async fn create_customer(&self, input: CreateCustomerInput) -> Result<domain::Customer, ApiError> {
        match self {
            Self::Mock(api) => api.create_customer(input).await,
            Self::Http(api) => api.create_customer(input).await,
        }
    }

    pub async fn update_lead(&self, id: Uuid, input: UpdateLeadInput) -> Result<domain::Customer, ApiError> {
        match self {
            Self::Mock(api) => api.update_lead(id, input).await,
            Self::Http(api) => api.update_lead(id, input).await,
        }
    }

    pub async fn create_sale(&self, input: CreateSaleInput) -> Result<domain::PlotSale, ApiError> {
        match self {
            Self::Mock(api) => api.create_sale(input).await,
            Self::Http(api) => api.create_sale(input).await,
        }
    }

    pub async fn get_loan_account(&self, id: Uuid) -> Result<LoanAccountDetail, ApiError> {
        match self {
            Self::Mock(api) => api.get_loan_account(id).await,
            Self::Http(api) => api.get_loan_account(id).await,
        }
    }

    pub async fn record_payment(&self, input: RecordPaymentInput) -> Result<domain::Payment, ApiError> {
        match self {
            Self::Mock(api) => api.record_payment(input).await,
            Self::Http(api) => api.record_payment(input).await,
        }
    }

    pub async fn list_platform_organizations(
        &self,
    ) -> Result<Vec<PlatformOrganizationSummary>, ApiError> {
        match self {
            Self::Mock(api) => api.list_platform_organizations().await,
            Self::Http(api) => api.list_platform_organizations().await,
        }
    }

    pub async fn get_platform_organization(
        &self,
        id: Uuid,
    ) -> Result<PlatformOrganizationDetail, ApiError> {
        match self {
            Self::Mock(api) => api.get_platform_organization(id).await,
            Self::Http(api) => api.get_platform_organization(id).await,
        }
    }

    pub async fn deactivate_organization(&self, id: Uuid) -> Result<(), ApiError> {
        match self {
            Self::Mock(api) => api.deactivate_organization(id).await,
            Self::Http(api) => api.deactivate_organization(id).await,
        }
    }

    pub async fn reactivate_organization(&self, id: Uuid) -> Result<(), ApiError> {
        match self {
            Self::Mock(api) => api.reactivate_organization(id).await,
            Self::Http(api) => api.reactivate_organization(id).await,
        }
    }

    pub async fn list_quotations(
        &self,
        customer_id: Option<Uuid>,
    ) -> Result<Vec<QuotationSummary>, ApiError> {
        match self {
            Self::Mock(api) => api.list_quotations(customer_id).await,
            Self::Http(api) => api.list_quotations(customer_id).await,
        }
    }

    pub async fn get_quotation(&self, id: Uuid) -> Result<QuotationDetail, ApiError> {
        match self {
            Self::Mock(api) => api.get_quotation(id).await,
            Self::Http(api) => api.get_quotation(id).await,
        }
    }

    pub async fn create_quotation(
        &self,
        input: CreateQuotationInput,
    ) -> Result<domain::Quotation, ApiError> {
        match self {
            Self::Mock(api) => api.create_quotation(input).await,
            Self::Http(api) => api.create_quotation(input).await,
        }
    }

    pub async fn send_quotation(&self, id: Uuid) -> Result<domain::Quotation, ApiError> {
        match self {
            Self::Mock(api) => api.send_quotation(id).await,
            Self::Http(api) => api.send_quotation(id).await,
        }
    }

    pub async fn accept_quotation(&self, id: Uuid) -> Result<domain::Quotation, ApiError> {
        match self {
            Self::Mock(api) => api.accept_quotation(id).await,
            Self::Http(api) => api.accept_quotation(id).await,
        }
    }

    pub async fn reject_quotation(&self, id: Uuid) -> Result<domain::Quotation, ApiError> {
        match self {
            Self::Mock(api) => api.reject_quotation(id).await,
            Self::Http(api) => api.reject_quotation(id).await,
        }
    }

    pub async fn list_approvals(
        &self,
        status: Option<&str>,
    ) -> Result<Vec<ApprovalRequestSummary>, ApiError> {
        match self {
            Self::Mock(api) => api.list_approvals(status).await,
            Self::Http(api) => api.list_approvals(status).await,
        }
    }

    pub async fn approve_request(
        &self,
        id: Uuid,
        notes: Option<String>,
    ) -> Result<ApprovalRequestSummary, ApiError> {
        match self {
            Self::Mock(api) => api.approve_request(id, notes).await,
            Self::Http(api) => api.approve_request(id, notes).await,
        }
    }

    pub async fn reject_request(
        &self,
        id: Uuid,
        notes: Option<String>,
    ) -> Result<ApprovalRequestSummary, ApiError> {
        match self {
            Self::Mock(api) => api.reject_request(id, notes).await,
            Self::Http(api) => api.reject_request(id, notes).await,
        }
    }
}
