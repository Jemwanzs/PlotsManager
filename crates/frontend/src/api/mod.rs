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

    pub async fn dashboard_analytics(&self) -> Result<domain::DashboardAnalytics, ApiError> {
        match self {
            Self::Mock(api) => api.dashboard_analytics().await,
            Self::Http(api) => api.dashboard_analytics().await,
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

    pub async fn update_plot(
        &self,
        project_id: Uuid,
        plot_id: Uuid,
        input: domain::UpdatePlotInput,
    ) -> Result<domain::Plot, ApiError> {
        match self {
            Self::Mock(api) => api.update_plot(project_id, plot_id, input).await,
            Self::Http(api) => api.update_plot(project_id, plot_id, input).await,
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

    pub async fn sales_report(
        &self,
        project_id: Option<Uuid>,
        agent_id: Option<Uuid>,
        from: Option<chrono::NaiveDate>,
        to: Option<chrono::NaiveDate>,
    ) -> Result<SalesReport, ApiError> {
        match self {
            Self::Mock(api) => api.sales_report(project_id, agent_id, from, to).await,
            Self::Http(api) => api.sales_report(project_id, agent_id, from, to).await,
        }
    }

    pub async fn inventory_report(&self) -> Result<InventoryReport, ApiError> {
        match self {
            Self::Mock(api) => api.inventory_report().await,
            Self::Http(api) => api.inventory_report().await,
        }
    }

    pub async fn agent_performance_report(
        &self,
        from: Option<chrono::NaiveDate>,
        to: Option<chrono::NaiveDate>,
    ) -> Result<AgentPerformanceReport, ApiError> {
        match self {
            Self::Mock(api) => api.agent_performance_report(from, to).await,
            Self::Http(api) => api.agent_performance_report(from, to).await,
        }
    }

    pub async fn get_map_summary(&self, project_id: Uuid) -> Result<domain::ProjectMapSummary, ApiError> {
        match self {
            Self::Mock(api) => api.get_map_summary(project_id).await,
            Self::Http(api) => api.get_map_summary(project_id).await,
        }
    }

    pub async fn upload_map_image(
        &self,
        project_id: Uuid,
        file: web_sys::File,
    ) -> Result<domain::ProjectMapSummary, ApiError> {
        match self {
            Self::Mock(api) => api.upload_map_image(project_id, file).await,
            Self::Http(api) => api.upload_map_image(project_id, file).await,
        }
    }

    pub async fn update_map_polygons(
        &self,
        project_id: Uuid,
        polygons: domain::MapPolygons,
    ) -> Result<domain::ProjectMapSummary, ApiError> {
        match self {
            Self::Mock(api) => api.update_map_polygons(project_id, polygons).await,
            Self::Http(api) => api.update_map_polygons(project_id, polygons).await,
        }
    }

    pub fn map_image_url(&self, project_id: Uuid) -> String {
        match self {
            Self::Mock(api) => api.map_image_url(project_id),
            Self::Http(api) => api.map_image_url(project_id),
        }
    }

    pub async fn create_plot_for_map_feature(
        &self,
        project_id: Uuid,
        feature_id: &str,
        input: CreatePlotInput,
    ) -> Result<domain::ProjectMapSummary, ApiError> {
        match self {
            Self::Mock(api) => api.create_plot_for_map_feature(project_id, feature_id, input).await,
            Self::Http(api) => api.create_plot_for_map_feature(project_id, feature_id, input).await,
        }
    }

    pub async fn link_plot_to_map_feature(
        &self,
        project_id: Uuid,
        feature_id: &str,
        plot_id: Uuid,
    ) -> Result<domain::ProjectMapSummary, ApiError> {
        match self {
            Self::Mock(api) => api.link_plot_to_map_feature(project_id, feature_id, plot_id).await,
            Self::Http(api) => api.link_plot_to_map_feature(project_id, feature_id, plot_id).await,
        }
    }

    pub async fn unlink_map_feature(
        &self,
        project_id: Uuid,
        feature_id: &str,
    ) -> Result<domain::ProjectMapSummary, ApiError> {
        match self {
            Self::Mock(api) => api.unlink_map_feature(project_id, feature_id).await,
            Self::Http(api) => api.unlink_map_feature(project_id, feature_id).await,
        }
    }

    pub async fn bulk_create_plots(
        &self,
        project_id: Uuid,
        inputs: Vec<CreatePlotInput>,
    ) -> Result<domain::BulkImportResult, ApiError> {
        match self {
            Self::Mock(api) => api.bulk_create_plots(project_id, inputs).await,
            Self::Http(api) => api.bulk_create_plots(project_id, inputs).await,
        }
    }

    pub async fn bulk_create_customers(
        &self,
        inputs: Vec<CreateCustomerInput>,
    ) -> Result<domain::BulkImportResult, ApiError> {
        match self {
            Self::Mock(api) => api.bulk_create_customers(inputs).await,
            Self::Http(api) => api.bulk_create_customers(inputs).await,
        }
    }

    pub async fn bulk_create_sales(
        &self,
        inputs: Vec<domain::BulkSaleRow>,
    ) -> Result<domain::BulkImportResult, ApiError> {
        match self {
            Self::Mock(api) => api.bulk_create_sales(inputs).await,
            Self::Http(api) => api.bulk_create_sales(inputs).await,
        }
    }

    pub async fn get_settings(&self) -> Result<domain::OrganizationSettings, ApiError> {
        match self {
            Self::Mock(api) => api.get_settings().await,
            Self::Http(api) => api.get_settings().await,
        }
    }

    pub async fn update_settings(
        &self,
        input: domain::UpdateOrganizationSettingsInput,
    ) -> Result<domain::OrganizationSettings, ApiError> {
        match self {
            Self::Mock(api) => api.update_settings(input).await,
            Self::Http(api) => api.update_settings(input).await,
        }
    }

    /// Atomically issues and returns the next auto-generated number for
    /// `entity_type` ("plot" or "project") — used by the "Auto-generate"
    /// affordance on the create-plot/create-project forms.
    /// `project_code` is only meaningful for `entity_type: "plot"`.
    pub async fn next_number(
        &self,
        entity_type: &str,
        project_code: Option<&str>,
    ) -> Result<String, ApiError> {
        match self {
            Self::Mock(api) => api.next_number(entity_type, project_code).await,
            Self::Http(api) => api.next_number(entity_type, project_code).await,
        }
    }

    pub async fn list_loan_accounts(&self) -> Result<Vec<domain::LoanAccountSummary>, ApiError> {
        match self {
            Self::Mock(api) => api.list_loan_accounts().await,
            Self::Http(api) => api.list_loan_accounts().await,
        }
    }

    pub async fn list_roles(&self) -> Result<Vec<domain::Role>, ApiError> {
        match self {
            Self::Mock(api) => api.list_roles().await,
            Self::Http(api) => api.list_roles().await,
        }
    }

    pub async fn list_permissions(&self) -> Result<Vec<(String, String)>, ApiError> {
        match self {
            Self::Mock(api) => api.list_permissions().await,
            Self::Http(api) => api.list_permissions().await,
        }
    }

    pub async fn create_role(&self, input: domain::CreateRoleInput) -> Result<domain::Role, ApiError> {
        match self {
            Self::Mock(api) => api.create_role(input).await,
            Self::Http(api) => api.create_role(input).await,
        }
    }

    pub async fn update_role(
        &self,
        id: Uuid,
        input: domain::UpdateRoleInput,
    ) -> Result<domain::Role, ApiError> {
        match self {
            Self::Mock(api) => api.update_role(id, input).await,
            Self::Http(api) => api.update_role(id, input).await,
        }
    }

    pub async fn delete_role(&self, id: Uuid) -> Result<(), ApiError> {
        match self {
            Self::Mock(api) => api.delete_role(id).await,
            Self::Http(api) => api.delete_role(id).await,
        }
    }

    pub async fn list_users(&self) -> Result<Vec<domain::TenantUser>, ApiError> {
        match self {
            Self::Mock(api) => api.list_users().await,
            Self::Http(api) => api.list_users().await,
        }
    }

    pub async fn create_user(&self, input: domain::CreateUserInput) -> Result<domain::TenantUser, ApiError> {
        match self {
            Self::Mock(api) => api.create_user(input).await,
            Self::Http(api) => api.create_user(input).await,
        }
    }

    pub async fn update_user(
        &self,
        id: Uuid,
        input: domain::UpdateUserInput,
    ) -> Result<domain::TenantUser, ApiError> {
        match self {
            Self::Mock(api) => api.update_user(id, input).await,
            Self::Http(api) => api.update_user(id, input).await,
        }
    }

    pub async fn activate_user(&self, id: Uuid) -> Result<domain::TenantUser, ApiError> {
        match self {
            Self::Mock(api) => api.activate_user(id).await,
            Self::Http(api) => api.activate_user(id).await,
        }
    }

    pub async fn deactivate_user(&self, id: Uuid) -> Result<domain::TenantUser, ApiError> {
        match self {
            Self::Mock(api) => api.deactivate_user(id).await,
            Self::Http(api) => api.deactivate_user(id).await,
        }
    }

    pub async fn list_branches(&self) -> Result<Vec<domain::Branch>, ApiError> {
        match self {
            Self::Mock(api) => api.list_branches().await,
            Self::Http(api) => api.list_branches().await,
        }
    }

    pub async fn create_branch(&self, input: domain::CreateBranchInput) -> Result<domain::Branch, ApiError> {
        match self {
            Self::Mock(api) => api.create_branch(input).await,
            Self::Http(api) => api.create_branch(input).await,
        }
    }

    pub async fn update_branch(
        &self,
        id: Uuid,
        input: domain::UpdateBranchInput,
    ) -> Result<domain::Branch, ApiError> {
        match self {
            Self::Mock(api) => api.update_branch(id, input).await,
            Self::Http(api) => api.update_branch(id, input).await,
        }
    }

    pub async fn activate_branch(&self, id: Uuid) -> Result<domain::Branch, ApiError> {
        match self {
            Self::Mock(api) => api.activate_branch(id).await,
            Self::Http(api) => api.activate_branch(id).await,
        }
    }

    pub async fn deactivate_branch(&self, id: Uuid) -> Result<domain::Branch, ApiError> {
        match self {
            Self::Mock(api) => api.deactivate_branch(id).await,
            Self::Http(api) => api.deactivate_branch(id).await,
        }
    }

    pub async fn reset_user_password(
        &self,
        id: Uuid,
        input: domain::ResetPasswordInput,
    ) -> Result<domain::TenantUser, ApiError> {
        match self {
            Self::Mock(api) => api.reset_user_password(id, input).await,
            Self::Http(api) => api.reset_user_password(id, input).await,
        }
    }

    pub async fn revoke_user_sessions(&self, id: Uuid) -> Result<domain::TenantUser, ApiError> {
        match self {
            Self::Mock(api) => api.revoke_user_sessions(id).await,
            Self::Http(api) => api.revoke_user_sessions(id).await,
        }
    }

    pub async fn change_password(
        &self,
        input: domain::ChangePasswordInput,
    ) -> Result<domain::AuthSession, ApiError> {
        match self {
            Self::Mock(api) => api.change_password(input).await,
            Self::Http(api) => api.change_password(input).await,
        }
    }
}
