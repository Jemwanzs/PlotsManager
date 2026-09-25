//! Real-backend implementation, talking to `crates/backend` over HTTP via
//! `gloo-net`. Implements the exact same contract `api::mock` does (see
//! its module docs), so swapping `ApiClient::new_mock()` for
//! `ApiClient::new_http(...)` in `app.rs` is the only thing components
//! ever need — no UI change.
//!
//! The session token isn't threaded through every call by hand: `login`
//! stashes it in `token` and every other request reads it from there to
//! set the `Authorization` header. That's `Arc<Mutex<_>>` rather than the
//! more obvious `Rc<RefCell<_>>` because `leptos::prelude::provide_context`
//! requires `Send + Sync` even here in a single-threaded CSR app — wasm32
//! has no real threads, so the lock is never contended, just a bound to
//! satisfy. This cell is separate from `crate::auth::AuthSignal` (the
//! reactive session state components read) — it exists purely so the
//! HTTP layer can authorize requests without every call site passing a
//! token around.

use std::sync::{Arc, Mutex};

use chrono::NaiveDate;
use gloo_net::http::{Request, RequestBuilder, Response};
use serde::de::DeserializeOwned;
use serde::Serialize;
use uuid::Uuid;

use domain::{
    AgentPerformanceReport, ApiError, ApprovalRequestSummary, AuthSession, Branch,
    BulkImportResult, BulkSaleRow, ChangePasswordInput, CreateBranchInput, CreateCustomerInput,
    CreatePlotInput, CreateProjectInput, CreateQuotationInput, CreateRoleInput, CreateSaleInput,
    CreateUserInput, CustomerDetail, CustomerSummary, DashboardAnalytics, DashboardSummary,
    DecideApprovalInput, FinanceReceivablesBreakdown, GeneratedNumber, InventoryReport, LinkPlotInput, LoanAccountDetail,
    LoanAccountSummary, LoginInput, MapPolygons, OrganizationSettings, PermissionDef,
    PlatformOrganizationDetail, PlatformOrganizationSummary, PlotCommercialSummary, PlotWithColor,
    PostWaiverInput, ProjectMapSummary, ProjectSummary,
    QuotationDetail, QuotationSummary, RecordPaymentInput, RejectOrganizationInput,
    ResetPasswordInput, ReverseEntryInput, Role, SalesReport, SignupInput, SignupResult, TenantUser, TermsVersion,
    UpdateBranchInput, UpdateCustomerInput, UpdateLeadInput, UpdateMapPolygonsInput, UpdateOrganizationSettingsInput,
    UpdatePlotInput, UpdateRoleInput, UpdateTitleRecordInput, UpdateUserInput, UploadDocumentInput,
    CreateTitleRecordInput, TitleRecord, CreateMigrationBatchInput, MigrationBatch,
    MigrationCommitResult, MigrationStagingRow, UpdateMigrationRowInput, CancelSaleInput,
    RepossessSaleInput,
};

#[derive(Clone)]
pub struct HttpApi {
    base_url: String,
    token: Arc<Mutex<Option<String>>>,
}

impl HttpApi {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            token: Arc::new(Mutex::new(None)),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    fn authorize(&self, builder: RequestBuilder) -> RequestBuilder {
        match self.token.lock().unwrap().as_deref() {
            Some(token) => builder.header("Authorization", &format!("Bearer {token}")),
            None => builder,
        }
    }

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, ApiError> {
        let resp = self
            .authorize(Request::get(&self.url(path)))
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;
        Self::parse(resp).await
    }

    async fn post<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, ApiError> {
        let req = self
            .authorize(Request::post(&self.url(path)))
            .json(body)
            .map_err(|e| ApiError::Network(e.to_string()))?;
        let resp = req
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;
        Self::parse(resp).await
    }

    async fn put<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, ApiError> {
        let req = self
            .authorize(Request::put(&self.url(path)))
            .json(body)
            .map_err(|e| ApiError::Network(e.to_string()))?;
        let resp = req
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;
        Self::parse(resp).await
    }

    async fn delete<T: DeserializeOwned>(&self, path: &str) -> Result<T, ApiError> {
        let resp = self
            .authorize(Request::delete(&self.url(path)))
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;
        Self::parse(resp).await
    }

    /// Uploads a `File` the user picked in an `<input type="file">` as
    /// a `multipart/form-data` body — the one request in this client
    /// that isn't a plain JSON `post`. Built via `web_sys::FormData`
    /// rather than hand-assembling a multipart body: the browser's
    /// `fetch` sets the correct `Content-Type` (with boundary) itself
    /// when the body is a `FormData`, which manually setting the
    /// header would break.
    async fn post_file<T: DeserializeOwned>(
        &self,
        path: &str,
        field_name: &str,
        file: web_sys::File,
    ) -> Result<T, ApiError> {
        self.post_file_with_fields(path, field_name, file, &[]).await
    }

    /// Same as `post_file`, plus arbitrary plain-text form fields sent
    /// alongside the file (e.g. document metadata) — `crates/backend`'s
    /// multipart handlers read those by field name off the same
    /// `Multipart` stream as the file part.
    async fn post_file_with_fields<T: DeserializeOwned>(
        &self,
        path: &str,
        field_name: &str,
        file: web_sys::File,
        extra_fields: &[(&str, &str)],
    ) -> Result<T, ApiError> {
        let form = web_sys::FormData::new().map_err(|_| {
            ApiError::Network("couldn't build the upload".to_string())
        })?;
        form.append_with_blob_and_filename(field_name, &file, &file.name())
            .map_err(|_| ApiError::Network("couldn't attach the file".to_string()))?;
        for (name, value) in extra_fields {
            form.append_with_str(name, value)
                .map_err(|_| ApiError::Network("couldn't attach form field".to_string()))?;
        }

        let req = self
            .authorize(Request::post(&self.url(path)))
            .body(form)
            .map_err(|e| ApiError::Network(e.to_string()))?;
        let resp = req
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;
        Self::parse(resp).await
    }

    /// Maps a response to the wire contract's `ApiError` by status code —
    /// mirrors `crates/backend/src/error.rs`'s `AppError -> HTTP status`
    /// side of the same mapping.
    async fn parse<T: DeserializeOwned>(resp: Response) -> Result<T, ApiError> {
        let status = resp.status();
        if status == 200 {
            resp.json::<T>().await.map_err(|e| {
                ApiError::Network(format!("couldn't read the server's response: {e}"))
            })
        } else {
            let message = resp
                .json::<serde_json::Value>()
                .await
                .ok()
                .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_string))
                .unwrap_or_else(|| format!("request failed with status {status}"));
            Err(match status {
                401 => ApiError::Unauthenticated,
                404 => ApiError::NotFound,
                // The mock uses `InvalidCredentials(String)` as the
                // general "show this message to the user" bucket, not
                // just for login — matched here for the same UI code
                // (new_project.rs, new_customer.rs, login.rs) to work
                // unchanged against either backend.
                400 | 403 | 409 => ApiError::InvalidCredentials(message),
                _ => ApiError::Network(message),
            })
        }
    }

    pub async fn login(&self, email: &str, password: &str) -> Result<AuthSession, ApiError> {
        let input = LoginInput {
            email: email.to_string(),
            password: password.to_string(),
        };
        let session: AuthSession = self.post("/api/v1/auth/login", &input).await?;
        *self.token.lock().unwrap() = Some(session.token.clone());
        Ok(session)
    }

    pub async fn signup(&self, input: SignupInput) -> Result<SignupResult, ApiError> {
        self.post("/api/v1/auth/signup", &input).await
    }

    pub async fn dashboard_summary(&self) -> Result<DashboardSummary, ApiError> {
        self.get("/api/v1/dashboard").await
    }

    pub async fn dashboard_analytics(&self) -> Result<DashboardAnalytics, ApiError> {
        self.get("/api/v1/dashboard/analytics").await
    }

    pub async fn list_projects(&self) -> Result<Vec<ProjectSummary>, ApiError> {
        self.get("/api/v1/projects").await
    }

    pub async fn get_project(&self, id: Uuid) -> Result<domain::Project, ApiError> {
        self.get(&format!("/api/v1/projects/{id}")).await
    }

    pub async fn create_project(
        &self,
        input: CreateProjectInput,
    ) -> Result<domain::Project, ApiError> {
        self.post("/api/v1/projects", &input).await
    }

    pub async fn update_project_commission(
        &self,
        project_id: Uuid,
        input: domain::UpdateProjectCommissionInput,
    ) -> Result<domain::Project, ApiError> {
        self.put(&format!("/api/v1/projects/{project_id}/commission-rate"), &input).await
    }

    pub async fn list_plots(&self, project_id: Uuid) -> Result<Vec<PlotWithColor>, ApiError> {
        self.get(&format!("/api/v1/projects/{project_id}/plots"))
            .await
    }

    pub async fn create_plot(&self, input: CreatePlotInput) -> Result<domain::Plot, ApiError> {
        let path = format!("/api/v1/projects/{}/plots", input.project_id);
        self.post(&path, &input).await
    }

    pub async fn update_plot(
        &self,
        project_id: Uuid,
        plot_id: Uuid,
        input: UpdatePlotInput,
    ) -> Result<domain::Plot, ApiError> {
        let path = format!("/api/v1/projects/{project_id}/plots/{plot_id}");
        self.put(&path, &input).await
    }

    pub async fn get_plot_commercial_summary(
        &self,
        project_id: Uuid,
        plot_id: Uuid,
    ) -> Result<PlotCommercialSummary, ApiError> {
        self.get(&format!("/api/v1/projects/{project_id}/plots/{plot_id}/commercial-summary")).await
    }

    pub async fn list_customers(&self) -> Result<Vec<CustomerSummary>, ApiError> {
        self.get("/api/v1/customers").await
    }

    pub async fn get_customer(&self, id: Uuid) -> Result<CustomerDetail, ApiError> {
        self.get(&format!("/api/v1/customers/{id}")).await
    }

    pub async fn create_customer(
        &self,
        input: CreateCustomerInput,
    ) -> Result<domain::Customer, ApiError> {
        self.post("/api/v1/customers", &input).await
    }

    pub async fn update_customer(
        &self,
        id: Uuid,
        input: UpdateCustomerInput,
    ) -> Result<domain::Customer, ApiError> {
        self.put(&format!("/api/v1/customers/{id}"), &input).await
    }

    pub async fn update_lead(
        &self,
        id: Uuid,
        input: UpdateLeadInput,
    ) -> Result<domain::Customer, ApiError> {
        self.post(&format!("/api/v1/customers/{id}/stage"), &input)
            .await
    }

    pub async fn create_sale(&self, input: CreateSaleInput) -> Result<domain::PlotSale, ApiError> {
        self.post("/api/v1/sales", &input).await
    }

    pub async fn cancel_sale(&self, sale_id: Uuid, input: CancelSaleInput) -> Result<(), ApiError> {
        self.post(&format!("/api/v1/sales/{sale_id}/cancel"), &input).await
    }

    pub async fn repossess_sale(&self, sale_id: Uuid, input: RepossessSaleInput) -> Result<(), ApiError> {
        self.post(&format!("/api/v1/sales/{sale_id}/repossess"), &input).await
    }

    pub async fn reallocate_plot(&self, project_id: Uuid, plot_id: Uuid) -> Result<domain::Plot, ApiError> {
        self.put(&format!("/api/v1/projects/{project_id}/plots/{plot_id}/reallocate"), &()).await
    }

    pub async fn get_loan_account(&self, id: Uuid) -> Result<LoanAccountDetail, ApiError> {
        self.get(&format!("/api/v1/loan-accounts/{id}")).await
    }

    pub async fn record_payment(
        &self,
        input: RecordPaymentInput,
    ) -> Result<domain::Payment, ApiError> {
        let path = format!("/api/v1/loan-accounts/{}/payments", input.loan_account_id);
        self.post(&path, &input).await
    }

    pub async fn get_loan_statement(&self, id: Uuid) -> Result<domain::LoanStatement, ApiError> {
        self.get(&format!("/api/v1/loan-accounts/{id}/statement")).await
    }

    pub async fn preview_allocation(
        &self,
        id: Uuid,
        amount: rust_decimal::Decimal,
    ) -> Result<domain::PaymentAllocationPreview, ApiError> {
        self.get(&format!("/api/v1/loan-accounts/{id}/allocation-preview?amount={amount}")).await
    }

    pub async fn post_charge(
        &self,
        input: domain::PostChargeInput,
    ) -> Result<domain::LoanLedgerEntry, ApiError> {
        let path = format!("/api/v1/loan-accounts/{}/charges", input.loan_account_id);
        self.post(&path, &input).await
    }

    pub async fn post_waiver(
        &self,
        input: PostWaiverInput,
    ) -> Result<domain::LoanLedgerEntry, ApiError> {
        let path = format!("/api/v1/loan-accounts/{}/waivers", input.loan_account_id);
        self.post(&path, &input).await
    }

    pub async fn reverse_entry(
        &self,
        loan_account_id: Uuid,
        entry_id: Uuid,
        reason: String,
    ) -> Result<domain::LoanLedgerEntry, ApiError> {
        let path = format!("/api/v1/loan-accounts/{loan_account_id}/ledger-entries/{entry_id}/reverse");
        self.post(&path, &ReverseEntryInput { reason }).await
    }

    pub async fn list_platform_organizations(
        &self,
    ) -> Result<Vec<PlatformOrganizationSummary>, ApiError> {
        self.get("/api/v1/platform/organizations").await
    }

    pub async fn get_platform_organization(
        &self,
        id: Uuid,
    ) -> Result<PlatformOrganizationDetail, ApiError> {
        self.get(&format!("/api/v1/platform/organizations/{id}"))
            .await
    }

    pub async fn deactivate_organization(&self, id: Uuid) -> Result<(), ApiError> {
        let _: serde_json::Value = self
            .post(
                &format!("/api/v1/platform/organizations/{id}/deactivate"),
                &(),
            )
            .await?;
        Ok(())
    }

    pub async fn reactivate_organization(&self, id: Uuid) -> Result<(), ApiError> {
        let _: serde_json::Value = self
            .post(
                &format!("/api/v1/platform/organizations/{id}/reactivate"),
                &(),
            )
            .await?;
        Ok(())
    }

    pub async fn approve_organization(&self, id: Uuid) -> Result<PlatformOrganizationSummary, ApiError> {
        self.post(&format!("/api/v1/platform/organizations/{id}/approve"), &())
            .await
    }

    pub async fn reject_organization(
        &self,
        id: Uuid,
        reason: String,
    ) -> Result<PlatformOrganizationSummary, ApiError> {
        self.post(
            &format!("/api/v1/platform/organizations/{id}/reject"),
            &RejectOrganizationInput { reason },
        )
        .await
    }

    pub async fn current_terms(&self) -> Result<TermsVersion, ApiError> {
        self.get("/api/v1/terms/current").await
    }

    pub async fn list_quotations(
        &self,
        customer_id: Option<Uuid>,
    ) -> Result<Vec<QuotationSummary>, ApiError> {
        match customer_id {
            Some(id) => self.get(&format!("/api/v1/quotations?customer_id={id}")).await,
            None => self.get("/api/v1/quotations").await,
        }
    }

    pub async fn get_quotation(&self, id: Uuid) -> Result<QuotationDetail, ApiError> {
        self.get(&format!("/api/v1/quotations/{id}")).await
    }

    pub async fn create_quotation(
        &self,
        input: CreateQuotationInput,
    ) -> Result<domain::Quotation, ApiError> {
        self.post("/api/v1/quotations", &input).await
    }

    pub async fn send_quotation(&self, id: Uuid) -> Result<domain::Quotation, ApiError> {
        self.post(&format!("/api/v1/quotations/{id}/send"), &()).await
    }

    pub async fn accept_quotation(&self, id: Uuid) -> Result<domain::Quotation, ApiError> {
        self.post(&format!("/api/v1/quotations/{id}/accept"), &()).await
    }

    pub async fn reject_quotation(&self, id: Uuid) -> Result<domain::Quotation, ApiError> {
        self.post(&format!("/api/v1/quotations/{id}/reject"), &()).await
    }

    pub async fn list_approvals(
        &self,
        status: Option<&str>,
    ) -> Result<Vec<ApprovalRequestSummary>, ApiError> {
        match status {
            Some(status) => self.get(&format!("/api/v1/approvals?status={status}")).await,
            None => self.get("/api/v1/approvals").await,
        }
    }

    pub async fn approve_request(
        &self,
        id: Uuid,
        notes: Option<String>,
    ) -> Result<ApprovalRequestSummary, ApiError> {
        self.post(&format!("/api/v1/approvals/{id}/approve"), &DecideApprovalInput { notes })
            .await
    }

    pub async fn reject_request(
        &self,
        id: Uuid,
        notes: Option<String>,
    ) -> Result<ApprovalRequestSummary, ApiError> {
        self.post(&format!("/api/v1/approvals/{id}/reject"), &DecideApprovalInput { notes })
            .await
    }

    pub async fn sales_report(
        &self,
        project_id: Option<Uuid>,
        agent_id: Option<Uuid>,
        from: Option<NaiveDate>,
        to: Option<NaiveDate>,
    ) -> Result<SalesReport, ApiError> {
        let mut params = Vec::new();
        if let Some(id) = project_id {
            params.push(format!("project_id={id}"));
        }
        if let Some(id) = agent_id {
            params.push(format!("agent_id={id}"));
        }
        if let Some(from) = from {
            params.push(format!("from={from}"));
        }
        if let Some(to) = to {
            params.push(format!("to={to}"));
        }
        self.get(&format!("/api/v1/reports/sales?{}", params.join("&"))).await
    }

    pub async fn inventory_report(&self) -> Result<InventoryReport, ApiError> {
        self.get("/api/v1/reports/inventory").await
    }

    pub async fn agent_performance_report(
        &self,
        from: Option<NaiveDate>,
        to: Option<NaiveDate>,
    ) -> Result<AgentPerformanceReport, ApiError> {
        let mut params = Vec::new();
        if let Some(from) = from {
            params.push(format!("from={from}"));
        }
        if let Some(to) = to {
            params.push(format!("to={to}"));
        }
        self.get(&format!("/api/v1/reports/agents?{}", params.join("&"))).await
    }

    pub async fn get_map_summary(&self, project_id: Uuid) -> Result<ProjectMapSummary, ApiError> {
        self.get(&format!("/api/v1/projects/{project_id}/map")).await
    }

    pub async fn upload_map_image(
        &self,
        project_id: Uuid,
        file: web_sys::File,
    ) -> Result<ProjectMapSummary, ApiError> {
        self.post_file(&format!("/api/v1/projects/{project_id}/map"), "image", file)
            .await
    }

    pub async fn update_map_polygons(
        &self,
        project_id: Uuid,
        polygons: MapPolygons,
    ) -> Result<ProjectMapSummary, ApiError> {
        self.put(
            &format!("/api/v1/projects/{project_id}/map/polygons"),
            &UpdateMapPolygonsInput { polygons },
        )
        .await
    }

    pub async fn create_plot_for_map_feature(
        &self,
        project_id: Uuid,
        feature_id: &str,
        input: CreatePlotInput,
    ) -> Result<ProjectMapSummary, ApiError> {
        self.put(
            &format!("/api/v1/projects/{project_id}/map/features/{feature_id}/create-plot"),
            &input,
        )
        .await
    }

    pub async fn link_plot_to_map_feature(
        &self,
        project_id: Uuid,
        feature_id: &str,
        plot_id: Uuid,
    ) -> Result<ProjectMapSummary, ApiError> {
        self.put(
            &format!("/api/v1/projects/{project_id}/map/features/{feature_id}/link-plot"),
            &LinkPlotInput { plot_id },
        )
        .await
    }

    pub async fn unlink_map_feature(
        &self,
        project_id: Uuid,
        feature_id: &str,
    ) -> Result<ProjectMapSummary, ApiError> {
        self.put(
            &format!("/api/v1/projects/{project_id}/map/features/{feature_id}/unlink"),
            &(),
        )
        .await
    }

    /// Not async — just a URL an `<img>` tag can point at directly.
    /// `<img>` can't send an `Authorization` header, so the token
    /// rides along as a query param instead (see
    /// `crates/backend/src/routes/project_map.rs`'s module docs).
    pub fn map_image_url(&self, project_id: Uuid) -> String {
        let token = self.token.lock().unwrap().clone().unwrap_or_default();
        format!("{}/api/v1/projects/{project_id}/map/image?token={token}", self.base_url)
    }

    pub async fn list_documents(
        &self,
        entity_type: domain::DocumentEntityType,
        entity_id: Uuid,
    ) -> Result<Vec<domain::DocumentMeta>, ApiError> {
        let entity_type = serde_json::to_value(entity_type)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default();
        self.get(&format!(
            "/api/v1/documents?entity_type={entity_type}&entity_id={entity_id}"
        ))
        .await
    }

    pub async fn upload_document(
        &self,
        input: UploadDocumentInput,
        file: web_sys::File,
    ) -> Result<domain::DocumentMeta, ApiError> {
        let entity_type = serde_json::to_value(input.entity_type)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default();
        let entity_id = input.entity_id.to_string();
        let mut fields: Vec<(&str, &str)> = vec![
            ("entity_type", &entity_type),
            ("entity_id", &entity_id),
            ("document_type", &input.document_type),
        ];
        if let Some(n) = input.document_number.as_deref() {
            fields.push(("document_number", n));
        }
        let issue_date_str = input.issue_date.map(|d| d.to_string());
        if let Some(s) = issue_date_str.as_deref() {
            fields.push(("issue_date", s));
        }
        let expiry_date_str = input.expiry_date.map(|d| d.to_string());
        if let Some(s) = expiry_date_str.as_deref() {
            fields.push(("expiry_date", s));
        }
        if let Some(d) = input.description.as_deref() {
            fields.push(("description", d));
        }
        self.post_file_with_fields("/api/v1/documents", "file", file, &fields)
            .await
    }

    pub async fn delete_document(&self, id: Uuid) -> Result<(), ApiError> {
        self.delete(&format!("/api/v1/documents/{id}")).await
    }

    /// Not async — an `<a href>`/`<img>` URL, same token-in-query
    /// pattern as `map_image_url`.
    pub fn document_file_url(&self, id: Uuid) -> String {
        let token = self.token.lock().unwrap().clone().unwrap_or_default();
        format!("{}/api/v1/documents/{id}/file?token={token}", self.base_url)
    }

    pub async fn list_title_records(&self, plot_id: Uuid) -> Result<Vec<TitleRecord>, ApiError> {
        self.get(&format!("/api/v1/plots/{plot_id}/title-records")).await
    }

    pub async fn create_title_record(
        &self,
        plot_id: Uuid,
        input: CreateTitleRecordInput,
    ) -> Result<TitleRecord, ApiError> {
        self.post(&format!("/api/v1/plots/{plot_id}/title-records"), &input).await
    }

    pub async fn update_title_record(
        &self,
        id: Uuid,
        input: UpdateTitleRecordInput,
    ) -> Result<TitleRecord, ApiError> {
        self.put(&format!("/api/v1/title-records/{id}"), &input).await
    }

    pub async fn list_migration_batches(&self) -> Result<Vec<MigrationBatch>, ApiError> {
        self.get("/api/v1/migrations").await
    }

    pub async fn get_migration_batch(&self, id: Uuid) -> Result<MigrationBatch, ApiError> {
        self.get(&format!("/api/v1/migrations/{id}")).await
    }

    pub async fn create_migration_batch(
        &self,
        input: CreateMigrationBatchInput,
    ) -> Result<MigrationBatch, ApiError> {
        self.post("/api/v1/migrations", &input).await
    }

    pub async fn list_migration_rows(&self, batch_id: Uuid) -> Result<Vec<MigrationStagingRow>, ApiError> {
        self.get(&format!("/api/v1/migrations/{batch_id}/rows")).await
    }

    pub async fn update_migration_row(
        &self,
        batch_id: Uuid,
        row_id: Uuid,
        input: UpdateMigrationRowInput,
    ) -> Result<MigrationStagingRow, ApiError> {
        self.put(&format!("/api/v1/migrations/{batch_id}/rows/{row_id}"), &input).await
    }

    pub async fn commit_migration_batch(&self, batch_id: Uuid) -> Result<MigrationCommitResult, ApiError> {
        self.post(&format!("/api/v1/migrations/{batch_id}/commit"), &()).await
    }

    pub async fn bulk_create_plots(
        &self,
        project_id: Uuid,
        inputs: Vec<CreatePlotInput>,
    ) -> Result<BulkImportResult, ApiError> {
        self.post(&format!("/api/v1/projects/{project_id}/plots/bulk"), &inputs).await
    }

    pub async fn bulk_create_customers(
        &self,
        inputs: Vec<CreateCustomerInput>,
    ) -> Result<BulkImportResult, ApiError> {
        self.post("/api/v1/customers/bulk", &inputs).await
    }

    pub async fn bulk_create_sales(
        &self,
        inputs: Vec<BulkSaleRow>,
    ) -> Result<BulkImportResult, ApiError> {
        self.post("/api/v1/sales/bulk", &inputs).await
    }

    pub async fn get_settings(&self) -> Result<OrganizationSettings, ApiError> {
        self.get("/api/v1/settings").await
    }

    pub async fn update_settings(
        &self,
        input: UpdateOrganizationSettingsInput,
    ) -> Result<OrganizationSettings, ApiError> {
        self.put("/api/v1/settings", &input).await
    }

    pub async fn next_number(
        &self,
        entity_type: &str,
        project_code: Option<&str>,
    ) -> Result<String, ApiError> {
        let path = match project_code.filter(|c| !c.is_empty()) {
            Some(code) => {
                let encoded = js_sys::encode_uri_component(code);
                format!(
                    "/api/v1/settings/numbering/{entity_type}/next?project_code={}",
                    encoded.as_string().unwrap_or_default()
                )
            }
            None => format!("/api/v1/settings/numbering/{entity_type}/next"),
        };
        let generated: GeneratedNumber = self.post(&path, &()).await?;
        Ok(generated.number)
    }

    pub async fn list_loan_accounts(&self) -> Result<Vec<LoanAccountSummary>, ApiError> {
        self.get("/api/v1/finance/loan-accounts").await
    }

    pub async fn receivables_breakdown(&self) -> Result<FinanceReceivablesBreakdown, ApiError> {
        self.get("/api/v1/finance/receivables-breakdown").await
    }

    pub async fn list_roles(&self) -> Result<Vec<Role>, ApiError> {
        self.get("/api/v1/roles").await
    }

    pub async fn list_permissions(&self) -> Result<Vec<PermissionDef>, ApiError> {
        self.get("/api/v1/roles/permissions").await
    }

    pub async fn create_role(&self, input: CreateRoleInput) -> Result<Role, ApiError> {
        self.post("/api/v1/roles", &input).await
    }

    pub async fn update_role(&self, id: Uuid, input: UpdateRoleInput) -> Result<Role, ApiError> {
        self.put(&format!("/api/v1/roles/{id}"), &input).await
    }

    pub async fn delete_role(&self, id: Uuid) -> Result<(), ApiError> {
        self.delete(&format!("/api/v1/roles/{id}")).await
    }

    pub async fn list_users(&self) -> Result<Vec<TenantUser>, ApiError> {
        self.get("/api/v1/users").await
    }

    pub async fn create_user(&self, input: CreateUserInput) -> Result<TenantUser, ApiError> {
        self.post("/api/v1/users", &input).await
    }

    pub async fn update_user(&self, id: Uuid, input: UpdateUserInput) -> Result<TenantUser, ApiError> {
        self.put(&format!("/api/v1/users/{id}"), &input).await
    }

    pub async fn activate_user(&self, id: Uuid) -> Result<TenantUser, ApiError> {
        self.put(&format!("/api/v1/users/{id}/activate"), &()).await
    }

    pub async fn deactivate_user(&self, id: Uuid) -> Result<TenantUser, ApiError> {
        self.put(&format!("/api/v1/users/{id}/deactivate"), &()).await
    }

    pub async fn list_branches(&self) -> Result<Vec<Branch>, ApiError> {
        self.get("/api/v1/branches").await
    }

    pub async fn create_branch(&self, input: CreateBranchInput) -> Result<Branch, ApiError> {
        self.post("/api/v1/branches", &input).await
    }

    pub async fn update_branch(&self, id: Uuid, input: UpdateBranchInput) -> Result<Branch, ApiError> {
        self.put(&format!("/api/v1/branches/{id}"), &input).await
    }

    pub async fn activate_branch(&self, id: Uuid) -> Result<Branch, ApiError> {
        self.put(&format!("/api/v1/branches/{id}/activate"), &()).await
    }

    pub async fn deactivate_branch(&self, id: Uuid) -> Result<Branch, ApiError> {
        self.put(&format!("/api/v1/branches/{id}/deactivate"), &()).await
    }

    pub async fn reset_user_password(
        &self,
        id: Uuid,
        input: ResetPasswordInput,
    ) -> Result<TenantUser, ApiError> {
        self.put(&format!("/api/v1/users/{id}/reset-password"), &input).await
    }

    pub async fn revoke_user_sessions(&self, id: Uuid) -> Result<TenantUser, ApiError> {
        self.put(&format!("/api/v1/users/{id}/revoke-sessions"), &()).await
    }

    /// Issues a fresh session token as a side effect of a successful
    /// change (see `crates/backend/src/routes/account.rs`'s doc
    /// comment for why) — stash it the same way `login`/`signup` do, or
    /// every request right after this one would fail with a stale
    /// token that the server just invalidated.
    pub async fn change_password(&self, input: ChangePasswordInput) -> Result<AuthSession, ApiError> {
        let session: AuthSession = self.put("/api/v1/account/change-password", &input).await?;
        *self.token.lock().unwrap() = Some(session.token.clone());
        Ok(session)
    }
}
