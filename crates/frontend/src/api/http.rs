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

use gloo_net::http::{Request, RequestBuilder, Response};
use serde::de::DeserializeOwned;
use serde::Serialize;
use uuid::Uuid;

use domain::{
    ApiError, AuthSession, CreateCustomerInput, CreatePlotInput, CreateProjectInput,
    CreateSaleInput, CustomerDetail, CustomerSummary, DashboardSummary, LoanAccountDetail,
    LoginInput, PlotWithColor, ProjectSummary, RecordPaymentInput,
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
                400 | 409 => ApiError::InvalidCredentials(message),
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

    pub async fn dashboard_summary(&self) -> Result<DashboardSummary, ApiError> {
        self.get("/api/v1/dashboard").await
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

    pub async fn list_plots(&self, project_id: Uuid) -> Result<Vec<PlotWithColor>, ApiError> {
        self.get(&format!("/api/v1/projects/{project_id}/plots"))
            .await
    }

    pub async fn create_plot(&self, input: CreatePlotInput) -> Result<domain::Plot, ApiError> {
        let path = format!("/api/v1/projects/{}/plots", input.project_id);
        self.post(&path, &input).await
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

    pub async fn create_sale(&self, input: CreateSaleInput) -> Result<domain::PlotSale, ApiError> {
        self.post("/api/v1/sales", &input).await
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
}
