//! In-memory sample data standing in for the real Rust API while the UI
//! is built ahead of it (see docs/14-development-roadmap.md). Every
//! method here has the exact signature `api::http::HttpApi` will
//! eventually have, so swapping `ApiClient::new_mock()` for
//! `ApiClient::new_http(base_url)` at the one call site in `app.rs` is
//! the entire migration — no component touches this module directly.

use std::sync::{Arc, Mutex};

use std::collections::HashMap;

use chrono::{Datelike, NaiveDate, TimeZone, Utc};
use domain::{
    approval_status_meta, loan_status_meta, plot_status_meta as status_meta,
    quotation_status_meta, AgentPerformanceReport, AgentPerformanceRow, ApiError, ApprovalRequest,
    ApprovalRequestSummary, ApprovalStatus, AreaUnit, AuthSession, BulkSaleRow, CreateCustomerInput,
    CreatePlotInput, CreateProjectInput, CreateQuotationInput, CreateSaleInput, Customer,
    CustomerDetail, CustomerSaleView, CustomerSummary, DashboardSummary, InventoryReport,
    LeadStage, LoanAccountDetail, LoanAccountStatus, MapPolygons, Organization, Payment,
    PaymentMode, PaymentStatus, PlatformOrganizationDetail, PlatformOrganizationSummary, Plot,
    PlotLoanAccount, PlotSale, PlotStatus, PlotStatusCount, PlotWithColor, Project,
    ProjectInventoryRow, ProjectMapSummary, ProjectStatus, ProjectSummary, Quotation,
    QuotationDetail, QuotationStatus, QuotationSummary, RecordPaymentInput, SalesReport,
    SalesReportRow, SignupInput, UpdateLeadInput, User,
};
use rust_decimal::Decimal;
use uuid::Uuid;

const DEMO_EMAIL: &str = "admin@acaciagrove.example";
const DEMO_PASSWORD: &str = "password123";

struct MockDb {
    organization: Organization,
    date_format: String,
    timezone: String,
    plot_numbering: MockNumbering,
    project_numbering: MockNumbering,
    demo_user: User,
    projects: Vec<Project>,
    plots: Vec<Plot>,
    customers: Vec<Customer>,
    sales: Vec<PlotSale>,
    loan_accounts: Vec<PlotLoanAccount>,
    payments: Vec<Payment>,
    quotations: Vec<Quotation>,
    approval_requests: Vec<ApprovalRequest>,
    project_maps: HashMap<Uuid, MockProjectMap>,
    roles: Vec<domain::Role>,
    tenant_users: Vec<domain::TenantUser>,
    branches: Vec<domain::Branch>,
}

/// Mirrors one row of the real `numbering_sequences` table
/// (`database/migrations/0010_organization_settings.sql`) — see
/// `crates/backend/src/routes/settings.rs` for the real implementation
/// this stands in for.
#[derive(Clone)]
struct MockNumbering {
    prefix: String,
    include_year: bool,
    include_entity_code: bool,
    padding: u32,
    next_number: u32,
}

/// Mirrors the real `project_maps` table (see
/// `database/migrations/0009_project_map.sql`) — `image_url` is a
/// browser blob: URL (`web_sys::Url::create_object_url_with_blob`)
/// rather than stored bytes, since the mock never leaves this tab and
/// has no server to round-trip bytes through.
struct MockProjectMap {
    image_url: String,
    image_content_type: String,
    polygons: MapPolygons,
    updated_at: chrono::DateTime<Utc>,
}

// Arc<Mutex<..>>, not Rc<RefCell<..>>: Leptos 0.7's `provide_context`
// requires `Send + Sync` even in a single-threaded CSR app. The Mutex is
// never actually contended (wasm32 is single-threaded here), it just
// satisfies the bound.
#[derive(Clone)]
pub struct MockApi {
    db: Arc<Mutex<MockDb>>,
}

impl Default for MockApi {
    fn default() -> Self {
        Self::new()
    }
}

impl MockApi {
    pub fn new() -> Self {
        Self {
            db: Arc::new(Mutex::new(seed())),
        }
    }

    pub async fn login(&self, email: &str, password: &str) -> Result<AuthSession, ApiError> {
        settle(300).await;
        if email.trim().eq_ignore_ascii_case(DEMO_EMAIL) && password == DEMO_PASSWORD {
            Ok(AuthSession {
                token: "mock-session-token".to_string(),
                user: self.db.lock().unwrap().demo_user.clone(),
            })
        } else {
            Err(ApiError::InvalidCredentials(
                "That email/password combination doesn't match our records.".to_string(),
            ))
        }
    }

    /// `MockDb` models one fixed demo organization, not a list — a real
    /// multi-tenant signup can't be simulated here the way it works
    /// against the real backend (crates/backend/src/routes/auth.rs).
    /// Matches the real backend's contract (a pending application, not
    /// a live session) without touching shared state, so the signup
    /// screen is previewable against mock data the same way every
    /// other screen is.
    pub async fn signup(&self, input: SignupInput) -> Result<domain::SignupResult, ApiError> {
        settle(400).await;
        if input.admin_password.len() < 8 {
            return Err(ApiError::InvalidCredentials(
                "Password must be at least 8 characters.".to_string(),
            ));
        }
        if !input.terms_accepted {
            return Err(ApiError::InvalidCredentials(
                "You must accept the Terms & Conditions to register.".to_string(),
            ));
        }
        Ok(domain::SignupResult {
            organization_id: Uuid::new_v4(),
            message: "Your account is awaiting activation. You will be notified once your workspace has been approved.".to_string(),
        })
    }

    pub async fn current_terms(&self) -> Result<domain::TermsVersion, ApiError> {
        settle(100).await;
        Ok(domain::TermsVersion {
            id: Uuid::new_v4(),
            version_label: "1.0".to_string(),
            body: "These Terms & Conditions govern access to and use of Real Estate Manager (\"the Platform\"). By creating an organization account you agree to: (1) provide accurate registration information; (2) use the Platform only for lawful property/plot sales management; (3) keep your account credentials confidential; (4) accept that your workspace is subject to Platform Owner review and activation before use; (5) accept the subscription and billing terms presented at the time of your chosen package; (6) allow the Platform to preserve your organization's operational data according to its retention rules even if your subscription is suspended or terminated.".to_string(),
        })
    }

    pub async fn dashboard_summary(&self) -> Result<DashboardSummary, ApiError> {
        settle(150).await;
        let db = self.db.lock().unwrap();

        let total_plots = db.plots.len() as u32;
        let sold_or_booked: Vec<&Plot> = db
            .plots
            .iter()
            .filter(|p| matches!(p.status, PlotStatus::Sold | PlotStatus::Booked))
            .collect();
        let total_sales_value: Decimal = sold_or_booked.iter().map(|p| p.asking_price).sum();

        let active_loan_plots: Vec<&Plot> = db
            .plots
            .iter()
            .filter(|p| matches!(p.status, PlotStatus::Booked | PlotStatus::UnderApproval))
            .collect();
        let active_loan_book: Decimal = active_loan_plots.iter().map(|p| p.asking_price).sum();

        // No real arrears data exists yet (no plot_loan_accounts wired to
        // the mock) — split the active book 70/30 so the dashboard reads
        // realistically instead of showing an all-or-nothing split.
        let performing_count = (active_loan_plots.len() as u32 * 7).div_euclid(10);
        let non_performing_count = active_loan_plots.len() as u32 - performing_count;
        let performing_amount = active_loan_book * Decimal::new(7, 1);
        let non_performing_amount = active_loan_book - performing_amount;

        Ok(DashboardSummary {
            total_customers: db.customers.len() as u32,
            total_projects: db.projects.len() as u32,
            total_plots,
            total_sales_count: sold_or_booked.len() as u32,
            total_sales_value,
            active_loans_count: active_loan_plots.len() as u32,
            active_loan_book,
            performing_count,
            performing_amount,
            non_performing_count,
            non_performing_amount,
        })
    }

    pub async fn dashboard_analytics(&self) -> Result<domain::DashboardAnalytics, ApiError> {
        settle(200).await;
        let db = self.db.lock().unwrap();
        let now = Utc::now();
        let year_start = Utc.with_ymd_and_hms(now.year(), 1, 1, 0, 0, 0).single().unwrap_or(now);
        let quarter_start_month = ((now.month() - 1) / 3) * 3 + 1;
        let quarter_start = Utc
            .with_ymd_and_hms(now.year(), quarter_start_month, 1, 0, 0, 0)
            .single()
            .unwrap_or(now);
        let prior_year_start = Utc
            .with_ymd_and_hms(now.year() - 1, 1, 1, 0, 0, 0)
            .single()
            .unwrap_or(year_start - chrono::Duration::days(365));
        let prior_year_asof = prior_year_start + (now - year_start);

        let qtd_sales: Vec<&PlotSale> = db.sales.iter().filter(|s| s.created_at >= quarter_start).collect();
        let ytd_sales: Vec<&PlotSale> = db.sales.iter().filter(|s| s.created_at >= year_start).collect();
        let prior_ytd_value: Decimal = db
            .sales
            .iter()
            .filter(|s| s.created_at >= prior_year_start && s.created_at < prior_year_asof)
            .map(|s| s.agreed_price)
            .sum();

        let months_back_start = now.year() * 12 + now.month0() as i32 - 11;
        let mut monthly_trend = Vec::with_capacity(12);
        for i in 0..12 {
            let total_months = months_back_start + i;
            let month_start = Utc
                .with_ymd_and_hms(total_months.div_euclid(12), total_months.rem_euclid(12) as u32 + 1, 1, 0, 0, 0)
                .single()
                .unwrap_or(now);
            let next_total_months = total_months + 1;
            let month_end = Utc
                .with_ymd_and_hms(next_total_months.div_euclid(12), next_total_months.rem_euclid(12) as u32 + 1, 1, 0, 0, 0)
                .single()
                .unwrap_or(month_start);
            let in_month: Vec<&PlotSale> = db
                .sales
                .iter()
                .filter(|s| s.created_at >= month_start && s.created_at < month_end)
                .collect();
            monthly_trend.push(domain::MonthlySalesPoint {
                period_label: month_start.format("%b %Y").to_string(),
                sales_value: in_month.iter().map(|s| s.agreed_price).sum(),
                sales_count: in_month.len() as u32,
            });
        }

        let mut by_status: Vec<(PlotStatus, u32, Decimal)> = Vec::new();
        for plot in &db.plots {
            match by_status.iter_mut().find(|(s, _, _)| *s == plot.status) {
                Some(entry) => {
                    entry.1 += 1;
                    entry.2 += plot.asking_price;
                }
                None => by_status.push((plot.status, 1, plot.asking_price)),
            }
        }
        let inventory_by_status = by_status
            .into_iter()
            .map(|(status, count, value)| {
                let (label, color) = domain::plot_status_meta(status);
                domain::PlotStatusCount {
                    status,
                    status_label: label.to_string(),
                    status_color: color.to_string(),
                    count,
                    value,
                }
            })
            .collect();

        let mut by_project: Vec<(Uuid, String, u32, Decimal)> = Vec::new();
        for sale in &ytd_sales {
            let Some(plot) = db.plots.iter().find(|p| p.id == sale.plot_id) else { continue };
            let Some(project) = db.projects.iter().find(|p| p.id == plot.project_id) else { continue };
            match by_project.iter_mut().find(|(id, _, _, _)| *id == project.id) {
                Some(entry) => {
                    entry.2 += 1;
                    entry.3 += sale.agreed_price;
                }
                None => by_project.push((project.id, project.name.clone(), 1, sale.agreed_price)),
            }
        }
        by_project.sort_by(|a, b| b.3.cmp(&a.3));
        by_project.truncate(8);
        let sales_by_project = by_project
            .into_iter()
            .map(|(_, project_name, count, value)| domain::ProjectSalesSlice {
                project_name,
                sales_value: value,
                sales_count: count,
            })
            .collect();

        Ok(domain::DashboardAnalytics {
            qtd_sales_value: qtd_sales.iter().map(|s| s.agreed_price).sum(),
            qtd_sales_count: qtd_sales.len() as u32,
            ytd_sales_value: ytd_sales.iter().map(|s| s.agreed_price).sum(),
            ytd_sales_count: ytd_sales.len() as u32,
            prior_ytd_sales_value: prior_ytd_value,
            monthly_trend,
            inventory_by_status,
            sales_by_project,
        })
    }

    pub async fn list_projects(&self) -> Result<Vec<ProjectSummary>, ApiError> {
        settle(150).await;
        let db = self.db.lock().unwrap();
        Ok(db
            .projects
            .iter()
            .map(|project| {
                let plots: Vec<&Plot> = db.plots.iter().filter(|p| p.project_id == project.id).collect();
                ProjectSummary {
                    id: project.id,
                    name: project.name.clone(),
                    code: project.code.clone(),
                    location: project.location.clone(),
                    status: project.status,
                    total_plots: plots.len() as u32,
                    available_plots: plots
                        .iter()
                        .filter(|p| p.status == PlotStatus::Available)
                        .count() as u32,
                    sold_plots: plots.iter().filter(|p| p.status == PlotStatus::Sold).count() as u32,
                }
            })
            .collect())
    }

    /// Unique project code, org-wide — matches the real constraint
    /// (`projects.organization_id, code` unique, database/migrations/0001_init.sql),
    /// checked here too since the mock has no database to enforce it.
    pub async fn create_project(&self, input: CreateProjectInput) -> Result<Project, ApiError> {
        settle(300).await;
        let mut db = self.db.lock().unwrap();

        let name = input.name.trim().to_string();
        let code = input.code.trim().to_uppercase();
        if name.is_empty() || code.is_empty() || input.location.trim().is_empty() {
            return Err(ApiError::InvalidCredentials(
                "Enter a project name, code, and location.".to_string(),
            ));
        }
        if db.projects.iter().any(|p| p.code == code) {
            return Err(ApiError::InvalidCredentials(format!(
                "Project code \"{code}\" is already in use."
            )));
        }

        let project = Project {
            id: Uuid::new_v4(),
            organization_id: db.organization.id,
            branch_id: None,
            name,
            code,
            location: input.location.trim().to_string(),
            original_title_number: None,
            total_size: input.total_size,
            area_unit: input.area_unit,
            status: ProjectStatus::Planning,
            assigned_manager_id: Some(db.demo_user.id),
            created_at: Utc::now(),
        };
        db.projects.push(project.clone());
        Ok(project)
    }

    pub async fn get_project(&self, id: Uuid) -> Result<Project, ApiError> {
        settle(150).await;
        self.db
            .lock().unwrap()
            .projects
            .iter()
            .find(|p| p.id == id)
            .cloned()
            .ok_or(ApiError::NotFound)
    }

    pub async fn list_plots(&self, project_id: Uuid) -> Result<Vec<PlotWithColor>, ApiError> {
        settle(150).await;
        let db = self.db.lock().unwrap();
        Ok(db
            .plots
            .iter()
            .filter(|p| p.project_id == project_id)
            .map(|p| {
                let (label, color) = status_meta(p.status);
                PlotWithColor {
                    plot: p.clone(),
                    status_label: label.to_string(),
                    status_color: color.to_string(),
                }
            })
            .collect())
    }

    /// `plot_number` unique **within its project** — fixes the legacy
    /// system's global-uniqueness bug (docs/02 §3: two different projects
    /// couldn't reuse the same plot description at all).
    pub async fn create_plot(&self, input: CreatePlotInput) -> Result<Plot, ApiError> {
        settle(300).await;
        let mut db = self.db.lock().unwrap();

        if !db.projects.iter().any(|p| p.id == input.project_id) {
            return Err(ApiError::NotFound);
        }
        let plot_number = input.plot_number.trim().to_string();
        if plot_number.is_empty() {
            return Err(ApiError::InvalidCredentials(
                "Enter a plot number.".to_string(),
            ));
        }
        if input.asking_price <= Decimal::ZERO {
            return Err(ApiError::InvalidCredentials(
                "Enter an asking price greater than zero.".to_string(),
            ));
        }
        if input.minimum_price > input.asking_price {
            return Err(ApiError::InvalidCredentials(
                "Minimum price can't be higher than the asking price.".to_string(),
            ));
        }
        validate_dimensions_mock(input.side_1, input.side_2)?;
        let duplicate = db
            .plots
            .iter()
            .any(|p| p.project_id == input.project_id && p.plot_number == plot_number);
        if duplicate {
            return Err(ApiError::InvalidCredentials(format!(
                "Plot \"{plot_number}\" already exists in this project."
            )));
        }

        let plot = Plot {
            id: Uuid::new_v4(),
            project_id: input.project_id,
            plot_number,
            title_number: None,
            size: input.size,
            side_1: input.side_1,
            side_2: input.side_2,
            dimension_unit: "ft".to_string(),
            asking_price: input.asking_price,
            minimum_price: input.minimum_price,
            status: PlotStatus::Available,
            map_feature_id: None,
            assigned_customer_id: None,
            created_at: Utc::now(),
        };
        db.plots.push(plot.clone());
        Ok(plot)
    }

    pub async fn update_plot(
        &self,
        project_id: Uuid,
        plot_id: Uuid,
        input: domain::UpdatePlotInput,
    ) -> Result<Plot, ApiError> {
        settle(300).await;
        let mut db = self.db.lock().unwrap();

        let plot_number = input.plot_number.trim().to_string();
        if plot_number.is_empty() {
            return Err(ApiError::InvalidCredentials(
                "Enter a plot number.".to_string(),
            ));
        }
        if input.asking_price <= Decimal::ZERO {
            return Err(ApiError::InvalidCredentials(
                "Enter an asking price greater than zero.".to_string(),
            ));
        }
        if input.minimum_price > input.asking_price {
            return Err(ApiError::InvalidCredentials(
                "Minimum price can't be higher than the asking price.".to_string(),
            ));
        }
        validate_dimensions_mock(input.side_1, input.side_2)?;
        let duplicate = db.plots.iter().any(|p| {
            p.project_id == project_id && p.plot_number == plot_number && p.id != plot_id
        });
        if duplicate {
            return Err(ApiError::InvalidCredentials(format!(
                "Plot \"{plot_number}\" already exists in this project."
            )));
        }

        let plot = db
            .plots
            .iter_mut()
            .find(|p| p.id == plot_id && p.project_id == project_id)
            .ok_or(ApiError::NotFound)?;
        plot.plot_number = plot_number;
        plot.size = input.size;
        plot.side_1 = input.side_1;
        plot.side_2 = input.side_2;
        plot.asking_price = input.asking_price;
        plot.minimum_price = input.minimum_price;
        Ok(plot.clone())
    }

    pub async fn list_customers(&self) -> Result<Vec<CustomerSummary>, ApiError> {
        settle(150).await;
        let db = self.db.lock().unwrap();
        Ok(db
            .customers
            .iter()
            .map(|customer| CustomerSummary {
                customer: customer.clone(),
                plots_owned: db
                    .plots
                    .iter()
                    .filter(|p| p.assigned_customer_id == Some(customer.id))
                    .count() as u32,
            })
            .collect())
    }

    /// Mirrors a real, working legacy validation (docs/02 §6): reject a
    /// duplicate ID/passport number rather than silently allowing two
    /// customer records for the same person.
    pub async fn create_customer(&self, input: CreateCustomerInput) -> Result<Customer, ApiError> {
        settle(300).await;
        let mut db = self.db.lock().unwrap();

        let full_name = input.full_name.trim().to_string();
        if full_name.is_empty() {
            return Err(ApiError::InvalidCredentials(
                "Enter the customer's name.".to_string(),
            ));
        }

        if let Some(id_number) = input.id_number.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            let duplicate = db
                .customers
                .iter()
                .any(|c| c.id_number.as_deref() == Some(id_number));
            if duplicate {
                return Err(ApiError::InvalidCredentials(
                    "That ID/passport number is already registered.".to_string(),
                ));
            }
        }

        let customer = Customer {
            id: Uuid::new_v4(),
            organization_id: db.organization.id,
            full_name,
            email: input.email.filter(|s| !s.trim().is_empty()),
            phone: input.phone.filter(|s| !s.trim().is_empty()),
            id_number: input.id_number.filter(|s| !s.trim().is_empty()),
            assigned_agent_id: Some(db.demo_user.id),
            stage: LeadStage::New,
            source: input.source.filter(|s| !s.trim().is_empty()),
            next_follow_up_at: None,
            notes: None,
            created_at: Utc::now(),
        };
        db.customers.push(customer.clone());
        Ok(customer)
    }

    pub async fn update_lead(&self, id: Uuid, input: UpdateLeadInput) -> Result<Customer, ApiError> {
        settle(200).await;
        let mut db = self.db.lock().unwrap();
        let customer = db
            .customers
            .iter_mut()
            .find(|c| c.id == id)
            .ok_or(ApiError::NotFound)?;
        customer.stage = input.stage;
        customer.next_follow_up_at = input.next_follow_up_at;
        customer.notes = input.notes.filter(|s| !s.trim().is_empty());
        Ok(customer.clone())
    }

    pub async fn get_customer(&self, id: Uuid) -> Result<CustomerDetail, ApiError> {
        settle(150).await;
        let db = self.db.lock().unwrap();
        let customer = db
            .customers
            .iter()
            .find(|c| c.id == id)
            .cloned()
            .ok_or(ApiError::NotFound)?;

        let sales = db
            .sales
            .iter()
            .filter(|s| s.customer_id == id)
            .filter_map(|sale| {
                let plot = db.plots.iter().find(|p| p.id == sale.plot_id)?;
                let project = db.projects.iter().find(|pr| pr.id == plot.project_id)?;
                let (label, color) = status_meta(plot.status);
                let loan_account_id = db
                    .loan_accounts
                    .iter()
                    .find(|la| la.sale_id == sale.id)
                    .map(|la| la.id);
                Some(CustomerSaleView {
                    sale_id: sale.id,
                    plot_id: plot.id,
                    project_id: project.id,
                    plot_number: plot.plot_number.clone(),
                    project_name: project.name.clone(),
                    payment_mode: sale.payment_mode,
                    agreed_price: sale.agreed_price,
                    status_label: label.to_string(),
                    status_color: color.to_string(),
                    loan_account_id,
                })
            })
            .collect();

        Ok(CustomerDetail { customer, sales })
    }

    /// Reserves a plot for a customer — the first step of the sales
    /// workflow (docs/07). Enforces the same invariant the schema does
    /// (`plot_sales_one_active_per_plot`, database/migrations/0001_init.sql):
    /// a plot already tied to an active sale can't be sold again.
    pub async fn create_sale(&self, input: CreateSaleInput) -> Result<PlotSale, ApiError> {
        settle(300).await;
        let mut db = self.db.lock().unwrap();

        let approval_id = gate_price_locked(
            &mut db,
            input.plot_id,
            input.customer_id,
            input.payment_mode,
            input.agreed_price,
            None,
        )?;

        let sale = execute_sale_locked(
            &mut db,
            input.plot_id,
            input.customer_id,
            input.payment_mode,
            input.agreed_price,
        )?;

        if let Some(approval_id) = approval_id {
            if let Some(req) = db.approval_requests.iter_mut().find(|r| r.id == approval_id) {
                req.resulting_sale_id = Some(sale.id);
            }
        }

        Ok(sale)
    }

    pub async fn get_loan_account(&self, id: Uuid) -> Result<LoanAccountDetail, ApiError> {
        settle(150).await;
        let db = self.db.lock().unwrap();
        let account = db
            .loan_accounts
            .iter()
            .find(|la| la.id == id)
            .cloned()
            .ok_or(ApiError::NotFound)?;
        let sale = db
            .sales
            .iter()
            .find(|s| s.id == account.sale_id)
            .ok_or(ApiError::NotFound)?;
        let plot = db
            .plots
            .iter()
            .find(|p| p.id == sale.plot_id)
            .ok_or(ApiError::NotFound)?;
        let project = db
            .projects
            .iter()
            .find(|p| p.id == plot.project_id)
            .ok_or(ApiError::NotFound)?;
        let customer = db
            .customers
            .iter()
            .find(|c| c.id == sale.customer_id)
            .ok_or(ApiError::NotFound)?;
        let (label, color) = loan_status_meta(account.status);

        let mut payments: Vec<Payment> = db
            .payments
            .iter()
            .filter(|p| p.loan_account_id == id)
            .cloned()
            .collect();
        payments.sort_by(|a, b| b.payment_date.cmp(&a.payment_date));

        Ok(LoanAccountDetail {
            plot_id: plot.id,
            plot_number: plot.plot_number.clone(),
            project_id: project.id,
            project_name: project.name.clone(),
            customer_id: customer.id,
            customer_name: customer.full_name.clone(),
            status_label: label.to_string(),
            status_color: color.to_string(),
            account,
            payments,
        })
    }

    /// Finance → Loan Accounts: every receivable across every project,
    /// org-wide — the list `get_loan_account` above has no equivalent
    /// for, since until the Finance module nothing needed one.
    pub async fn list_loan_accounts(&self) -> Result<Vec<domain::LoanAccountSummary>, ApiError> {
        settle(200).await;
        let db = self.db.lock().unwrap();
        let mut summaries: Vec<domain::LoanAccountSummary> = db
            .loan_accounts
            .iter()
            .filter_map(|account| {
                let sale = db.sales.iter().find(|s| s.id == account.sale_id)?;
                let plot = db.plots.iter().find(|p| p.id == sale.plot_id)?;
                let project = db.projects.iter().find(|p| p.id == plot.project_id)?;
                let customer = db.customers.iter().find(|c| c.id == sale.customer_id)?;
                let (label, color) = loan_status_meta(account.status);
                Some(domain::LoanAccountSummary {
                    account: account.clone(),
                    plot_id: plot.id,
                    plot_number: plot.plot_number.clone(),
                    project_id: project.id,
                    project_name: project.name.clone(),
                    customer_id: customer.id,
                    customer_name: customer.full_name.clone(),
                    status_label: label.to_string(),
                    status_color: color.to_string(),
                })
            })
            .collect();
        summaries.sort_by(|a, b| {
            b.account
                .outstanding_balance
                .cmp(&a.account.outstanding_balance)
        });
        Ok(summaries)
    }

    pub async fn list_roles(&self) -> Result<Vec<domain::Role>, ApiError> {
        settle(150).await;
        let db = self.db.lock().unwrap();
        Ok(db.roles.clone())
    }

    pub async fn list_permissions(&self) -> Result<Vec<(String, String)>, ApiError> {
        settle(100).await;
        Ok(domain::ALL_PERMISSIONS
            .iter()
            .map(|(perm, label)| (perm.to_string(), label.to_string()))
            .collect())
    }

    pub async fn create_role(&self, input: domain::CreateRoleInput) -> Result<domain::Role, ApiError> {
        settle(250).await;
        let mut db = self.db.lock().unwrap();
        let name = input.name.trim().to_string();
        if name.is_empty() {
            return Err(ApiError::InvalidCredentials("Enter a role name.".to_string()));
        }
        if db.roles.iter().any(|r| r.name.eq_ignore_ascii_case(&name)) {
            return Err(ApiError::InvalidCredentials(format!(
                "A role named \"{name}\" already exists."
            )));
        }
        let role = domain::Role {
            id: Uuid::new_v4(),
            organization_id: db.organization.id,
            name,
            permissions: input.permissions,
            assigned_user_count: 0,
        };
        db.roles.push(role.clone());
        Ok(role)
    }

    pub async fn update_role(
        &self,
        id: Uuid,
        input: domain::UpdateRoleInput,
    ) -> Result<domain::Role, ApiError> {
        settle(250).await;
        let mut db = self.db.lock().unwrap();
        let name = input.name.trim().to_string();
        if name.is_empty() {
            return Err(ApiError::InvalidCredentials("Enter a role name.".to_string()));
        }
        if db.roles.iter().any(|r| r.id != id && r.name.eq_ignore_ascii_case(&name)) {
            return Err(ApiError::InvalidCredentials(format!(
                "A role named \"{name}\" already exists."
            )));
        }
        let role = db.roles.iter_mut().find(|r| r.id == id).ok_or(ApiError::NotFound)?;
        role.name = name;
        role.permissions = input.permissions;
        Ok(role.clone())
    }

    pub async fn delete_role(&self, id: Uuid) -> Result<(), ApiError> {
        settle(200).await;
        let mut db = self.db.lock().unwrap();
        let role = db.roles.iter().find(|r| r.id == id).ok_or(ApiError::NotFound)?;
        if role.assigned_user_count > 0 {
            return Err(ApiError::InvalidCredentials(format!(
                "This role is still assigned to {} user(s) — reassign them first.",
                role.assigned_user_count
            )));
        }
        db.roles.retain(|r| r.id != id);
        Ok(())
    }

    pub async fn list_users(&self) -> Result<Vec<domain::TenantUser>, ApiError> {
        settle(150).await;
        let db = self.db.lock().unwrap();
        Ok(db.tenant_users.clone())
    }

    pub async fn create_user(&self, input: domain::CreateUserInput) -> Result<domain::TenantUser, ApiError> {
        settle(250).await;
        let mut db = self.db.lock().unwrap();
        let full_name = input.full_name.trim().to_string();
        let email = input.email.trim().to_string();
        if full_name.is_empty() {
            return Err(ApiError::InvalidCredentials("Enter a name.".to_string()));
        }
        if email.is_empty() {
            return Err(ApiError::InvalidCredentials("Enter an email.".to_string()));
        }
        if input.temporary_password.len() < 8 {
            return Err(ApiError::InvalidCredentials(
                "Temporary password must be at least 8 characters.".to_string(),
            ));
        }
        let email_taken = db.tenant_users.iter().any(|u| u.email.eq_ignore_ascii_case(&email))
            || db.demo_user.email.eq_ignore_ascii_case(&email);
        if email_taken {
            return Err(ApiError::InvalidCredentials(
                "An account with that email already exists.".to_string(),
            ));
        }
        let Some(role_name) = db.roles.iter().find(|r| r.id == input.role_id).map(|r| r.name.clone()) else {
            return Err(ApiError::InvalidCredentials("Choose a valid role.".to_string()));
        };
        let mut branch_ids = input.branch_ids.clone();
        branch_ids.dedup();
        let branch_name = branch_ids
            .first()
            .and_then(|bid| db.branches.iter().find(|b| b.id == *bid).map(|b| b.name.clone()));

        let user = domain::TenantUser {
            id: Uuid::new_v4(),
            full_name,
            email,
            mobile: input.mobile.map(|m| m.trim().to_string()).filter(|m| !m.is_empty()),
            is_active: true,
            is_platform_owner: false,
            branch_id: branch_ids.first().copied(),
            branch_name,
            branch_count: branch_ids.len() as i64,
            branch_ids,
            role_id: Some(input.role_id),
            role_name: Some(role_name),
            last_login_at: None,
            created_at: Utc::now(),
            must_change_password: true,
            password_changed_at: Utc::now(),
        };
        db.tenant_users.push(user.clone());
        if let Some(role) = db.roles.iter_mut().find(|r| r.id == input.role_id) {
            role.assigned_user_count += 1;
        }
        Ok(user)
    }

    pub async fn update_user(&self, id: Uuid, input: domain::UpdateUserInput) -> Result<domain::TenantUser, ApiError> {
        settle(250).await;
        let mut db = self.db.lock().unwrap();
        let full_name = input.full_name.trim().to_string();
        let email = input.email.trim().to_string();
        if full_name.is_empty() {
            return Err(ApiError::InvalidCredentials("Enter a name.".to_string()));
        }
        if email.is_empty() {
            return Err(ApiError::InvalidCredentials("Enter an email.".to_string()));
        }
        let email_taken = db
            .tenant_users
            .iter()
            .any(|u| u.id != id && u.email.eq_ignore_ascii_case(&email));
        if email_taken {
            return Err(ApiError::InvalidCredentials(
                "An account with that email already exists.".to_string(),
            ));
        }
        let Some(role_name) = db.roles.iter().find(|r| r.id == input.role_id).map(|r| r.name.clone()) else {
            return Err(ApiError::InvalidCredentials("Choose a valid role.".to_string()));
        };
        let mut branch_ids = input.branch_ids.clone();
        branch_ids.dedup();
        let branch_name = branch_ids
            .first()
            .and_then(|bid| db.branches.iter().find(|b| b.id == *bid).map(|b| b.name.clone()));

        let old_role_id = db.tenant_users.iter().find(|u| u.id == id).and_then(|u| u.role_id);

        let user = db.tenant_users.iter_mut().find(|u| u.id == id).ok_or(ApiError::NotFound)?;
        user.full_name = full_name;
        user.email = email;
        user.mobile = input.mobile.map(|m| m.trim().to_string()).filter(|m| !m.is_empty());
        user.branch_id = branch_ids.first().copied();
        user.branch_name = branch_name;
        user.branch_count = branch_ids.len() as i64;
        user.branch_ids = branch_ids;
        user.role_id = Some(input.role_id);
        user.role_name = Some(role_name);
        let result = user.clone();

        if old_role_id != Some(input.role_id) {
            if let Some(old_id) = old_role_id {
                if let Some(r) = db.roles.iter_mut().find(|r| r.id == old_id) {
                    r.assigned_user_count = r.assigned_user_count.saturating_sub(1);
                }
            }
            if let Some(r) = db.roles.iter_mut().find(|r| r.id == input.role_id) {
                r.assigned_user_count += 1;
            }
        }
        Ok(result)
    }

    pub async fn activate_user(&self, id: Uuid) -> Result<domain::TenantUser, ApiError> {
        settle(150).await;
        let mut db = self.db.lock().unwrap();
        let user = db.tenant_users.iter_mut().find(|u| u.id == id).ok_or(ApiError::NotFound)?;
        user.is_active = true;
        Ok(user.clone())
    }

    pub async fn deactivate_user(&self, id: Uuid) -> Result<domain::TenantUser, ApiError> {
        settle(150).await;
        let mut db = self.db.lock().unwrap();
        if id == db.demo_user.id {
            return Err(ApiError::InvalidCredentials(
                "You can't deactivate your own account.".to_string(),
            ));
        }
        let user = db.tenant_users.iter_mut().find(|u| u.id == id).ok_or(ApiError::NotFound)?;
        user.is_active = false;
        Ok(user.clone())
    }

    pub async fn list_branches(&self) -> Result<Vec<domain::Branch>, ApiError> {
        settle(100).await;
        let db = self.db.lock().unwrap();
        Ok(db.branches.clone())
    }

    pub async fn create_branch(&self, input: domain::CreateBranchInput) -> Result<domain::Branch, ApiError> {
        settle(250).await;
        let mut db = self.db.lock().unwrap();
        let name = input.name.trim().to_string();
        let code = input.code.trim().to_uppercase();
        if name.is_empty() {
            return Err(ApiError::InvalidCredentials("Enter a branch name.".to_string()));
        }
        if code.is_empty() {
            return Err(ApiError::InvalidCredentials("Enter a branch code.".to_string()));
        }
        if db.branches.iter().any(|b| b.code == code) {
            return Err(ApiError::InvalidCredentials(format!(
                "A branch with code \"{code}\" already exists."
            )));
        }
        let manager_name = input
            .manager_id
            .and_then(|mid| db.tenant_users.iter().find(|u| u.id == mid).map(|u| u.full_name.clone()));
        let branch = domain::Branch {
            id: Uuid::new_v4(),
            organization_id: db.organization.id,
            name,
            code,
            region: input.region.map(|r| r.trim().to_string()).filter(|r| !r.is_empty()),
            location: input.location.map(|r| r.trim().to_string()).filter(|r| !r.is_empty()),
            contact_name: input.contact_name.map(|r| r.trim().to_string()).filter(|r| !r.is_empty()),
            contact_phone: input.contact_phone.map(|r| r.trim().to_string()).filter(|r| !r.is_empty()),
            manager_id: input.manager_id,
            manager_name,
            is_active: true,
        };
        db.branches.push(branch.clone());
        Ok(branch)
    }

    pub async fn update_branch(
        &self,
        id: Uuid,
        input: domain::UpdateBranchInput,
    ) -> Result<domain::Branch, ApiError> {
        settle(250).await;
        let mut db = self.db.lock().unwrap();
        let name = input.name.trim().to_string();
        let code = input.code.trim().to_uppercase();
        if name.is_empty() {
            return Err(ApiError::InvalidCredentials("Enter a branch name.".to_string()));
        }
        if code.is_empty() {
            return Err(ApiError::InvalidCredentials("Enter a branch code.".to_string()));
        }
        if db.branches.iter().any(|b| b.id != id && b.code == code) {
            return Err(ApiError::InvalidCredentials(format!(
                "A branch with code \"{code}\" already exists."
            )));
        }
        let manager_name = input
            .manager_id
            .and_then(|mid| db.tenant_users.iter().find(|u| u.id == mid).map(|u| u.full_name.clone()));
        let branch = db.branches.iter_mut().find(|b| b.id == id).ok_or(ApiError::NotFound)?;
        branch.name = name;
        branch.code = code;
        branch.region = input.region.map(|r| r.trim().to_string()).filter(|r| !r.is_empty());
        branch.location = input.location.map(|r| r.trim().to_string()).filter(|r| !r.is_empty());
        branch.contact_name = input.contact_name.map(|r| r.trim().to_string()).filter(|r| !r.is_empty());
        branch.contact_phone = input.contact_phone.map(|r| r.trim().to_string()).filter(|r| !r.is_empty());
        branch.manager_id = input.manager_id;
        branch.manager_name = manager_name;
        Ok(branch.clone())
    }

    pub async fn activate_branch(&self, id: Uuid) -> Result<domain::Branch, ApiError> {
        settle(150).await;
        let mut db = self.db.lock().unwrap();
        let branch = db.branches.iter_mut().find(|b| b.id == id).ok_or(ApiError::NotFound)?;
        branch.is_active = true;
        Ok(branch.clone())
    }

    pub async fn deactivate_branch(&self, id: Uuid) -> Result<domain::Branch, ApiError> {
        settle(150).await;
        let mut db = self.db.lock().unwrap();
        let branch = db.branches.iter_mut().find(|b| b.id == id).ok_or(ApiError::NotFound)?;
        branch.is_active = false;
        Ok(branch.clone())
    }

    pub async fn reset_user_password(
        &self,
        id: Uuid,
        input: domain::ResetPasswordInput,
    ) -> Result<domain::TenantUser, ApiError> {
        settle(250).await;
        if input.temporary_password.len() < 8 {
            return Err(ApiError::InvalidCredentials(
                "Temporary password must be at least 8 characters.".to_string(),
            ));
        }
        let mut db = self.db.lock().unwrap();
        let user = db.tenant_users.iter_mut().find(|u| u.id == id).ok_or(ApiError::NotFound)?;
        user.must_change_password = true;
        user.password_changed_at = Utc::now();
        Ok(user.clone())
    }

    pub async fn revoke_user_sessions(&self, id: Uuid) -> Result<domain::TenantUser, ApiError> {
        settle(150).await;
        let db = self.db.lock().unwrap();
        db.tenant_users.iter().find(|u| u.id == id).cloned().ok_or(ApiError::NotFound)
    }

    /// Mock login only ever authenticates the single seeded `demo_user`
    /// (see `login` above), so this can only meaningfully act on that
    /// account — matches the same limitation.
    pub async fn change_password(
        &self,
        input: domain::ChangePasswordInput,
    ) -> Result<AuthSession, ApiError> {
        settle(300).await;
        if input.current_password != DEMO_PASSWORD {
            return Err(ApiError::InvalidCredentials(
                "Your current password is incorrect.".to_string(),
            ));
        }
        if input.new_password.len() < 8 {
            return Err(ApiError::InvalidCredentials(
                "New password must be at least 8 characters.".to_string(),
            ));
        }
        let mut db = self.db.lock().unwrap();
        db.demo_user.must_change_password = false;
        let demo_id = db.demo_user.id;
        if let Some(tu) = db.tenant_users.iter_mut().find(|u| u.id == demo_id) {
            tu.must_change_password = false;
            tu.password_changed_at = Utc::now();
        }
        Ok(AuthSession {
            token: "mock-session-token".to_string(),
            user: db.demo_user.clone(),
        })
    }

    /// Records a payment against a Plot Loan Account and updates its
    /// running balance/status. Posted immediately — the
    /// Captured/Verified/Posted lifecycle and approval gating from
    /// docs/08 apply once real authenticated users and an approval engine
    /// exist (docs/09); this mock has neither yet.
    pub async fn record_payment(&self, input: RecordPaymentInput) -> Result<Payment, ApiError> {
        settle(300).await;
        let mut db = self.db.lock().unwrap();

        if input.amount <= Decimal::ZERO {
            return Err(ApiError::InvalidCredentials(
                "Enter an amount greater than zero.".to_string(),
            ));
        }

        let captured_by = db.demo_user.id;
        let payment = Payment {
            id: Uuid::new_v4(),
            loan_account_id: input.loan_account_id,
            amount: input.amount,
            payment_date: input.payment_date,
            method: input.method,
            external_reference: None,
            status: PaymentStatus::Posted,
            captured_by,
            verified_by: Some(captured_by),
            created_at: Utc::now(),
        };

        let account = db
            .loan_accounts
            .iter_mut()
            .find(|la| la.id == input.loan_account_id)
            .ok_or(ApiError::NotFound)?;
        account.amount_paid += input.amount;
        account.outstanding_balance = (account.outstanding_balance - input.amount).max(Decimal::ZERO);
        account.status = if account.outstanding_balance <= Decimal::ZERO {
            LoanAccountStatus::FullyPaid
        } else if account.amount_paid > Decimal::ZERO {
            LoanAccountStatus::ActivePartiallyPaid
        } else {
            account.status
        };

        db.payments.push(payment.clone());
        Ok(payment)
    }

    // MockDb's demo_user is never a platform owner (see `signup` above —
    // there's no multi-tenant model to admin here), so these mirror the
    // real backend's 403 for a non-owner caller rather than simulating
    // real platform-admin data.
    pub async fn list_platform_organizations(
        &self,
    ) -> Result<Vec<PlatformOrganizationSummary>, ApiError> {
        settle(200).await;
        Err(ApiError::InvalidCredentials(
            "This account doesn't have platform administrator access.".to_string(),
        ))
    }

    pub async fn get_platform_organization(
        &self,
        _id: Uuid,
    ) -> Result<PlatformOrganizationDetail, ApiError> {
        settle(200).await;
        Err(ApiError::InvalidCredentials(
            "This account doesn't have platform administrator access.".to_string(),
        ))
    }

    pub async fn deactivate_organization(&self, _id: Uuid) -> Result<(), ApiError> {
        settle(200).await;
        Err(ApiError::InvalidCredentials(
            "This account doesn't have platform administrator access.".to_string(),
        ))
    }

    pub async fn reactivate_organization(&self, _id: Uuid) -> Result<(), ApiError> {
        settle(200).await;
        Err(ApiError::InvalidCredentials(
            "This account doesn't have platform administrator access.".to_string(),
        ))
    }

    pub async fn approve_organization(&self, _id: Uuid) -> Result<PlatformOrganizationSummary, ApiError> {
        settle(200).await;
        Err(ApiError::InvalidCredentials(
            "This account doesn't have platform administrator access.".to_string(),
        ))
    }

    pub async fn reject_organization(
        &self,
        _id: Uuid,
        _reason: String,
    ) -> Result<PlatformOrganizationSummary, ApiError> {
        settle(200).await;
        Err(ApiError::InvalidCredentials(
            "This account doesn't have platform administrator access.".to_string(),
        ))
    }

    pub async fn list_quotations(
        &self,
        customer_id: Option<Uuid>,
    ) -> Result<Vec<QuotationSummary>, ApiError> {
        settle(200).await;
        let db = self.db.lock().unwrap();
        Ok(db
            .quotations
            .iter()
            .filter(|q| customer_id.is_none_or(|id| q.customer_id == id))
            .filter_map(|q| quotation_summary(&db, q))
            .collect())
    }

    pub async fn get_quotation(&self, id: Uuid) -> Result<QuotationDetail, ApiError> {
        settle(150).await;
        let db = self.db.lock().unwrap();
        let quotation = db.quotations.iter().find(|q| q.id == id).ok_or(ApiError::NotFound)?;
        quotation_detail(&db, quotation).ok_or(ApiError::NotFound)
    }

    pub async fn create_quotation(&self, input: CreateQuotationInput) -> Result<Quotation, ApiError> {
        settle(300).await;
        let mut db = self.db.lock().unwrap();

        if input.quoted_price <= Decimal::ZERO {
            return Err(ApiError::InvalidCredentials(
                "Enter a quoted price greater than zero.".to_string(),
            ));
        }
        if !db.plots.iter().any(|p| p.id == input.plot_id) {
            return Err(ApiError::NotFound);
        }
        if !db.customers.iter().any(|c| c.id == input.customer_id) {
            return Err(ApiError::NotFound);
        }

        let quotation = Quotation {
            id: Uuid::new_v4(),
            organization_id: db.organization.id,
            plot_id: input.plot_id,
            customer_id: input.customer_id,
            agent_id: Some(db.demo_user.id),
            payment_mode: input.payment_mode,
            quoted_price: input.quoted_price,
            valid_until: input.valid_until,
            status: QuotationStatus::Draft,
            notes: input.notes.filter(|s| !s.trim().is_empty()),
            converted_sale_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        db.quotations.push(quotation.clone());
        Ok(quotation)
    }

    pub async fn send_quotation(&self, id: Uuid) -> Result<Quotation, ApiError> {
        settle(200).await;
        self.transition_quotation(id, QuotationStatus::Draft, QuotationStatus::Sent)
    }

    pub async fn reject_quotation(&self, id: Uuid) -> Result<Quotation, ApiError> {
        settle(200).await;
        self.transition_quotation(id, QuotationStatus::Sent, QuotationStatus::Rejected)
    }

    fn transition_quotation(
        &self,
        id: Uuid,
        from: QuotationStatus,
        to: QuotationStatus,
    ) -> Result<Quotation, ApiError> {
        let mut db = self.db.lock().unwrap();
        let quotation = db.quotations.iter_mut().find(|q| q.id == id).ok_or(ApiError::NotFound)?;
        if quotation.status != from {
            return Err(ApiError::InvalidCredentials(
                "This quotation isn't in the right state for that action anymore.".to_string(),
            ));
        }
        quotation.status = to;
        quotation.updated_at = Utc::now();
        Ok(quotation.clone())
    }

    pub async fn accept_quotation(&self, id: Uuid) -> Result<Quotation, ApiError> {
        settle(300).await;
        let mut db = self.db.lock().unwrap();

        let Some(quotation) = db.quotations.iter().find(|q| q.id == id).cloned() else {
            return Err(ApiError::NotFound);
        };
        if quotation.status != QuotationStatus::Sent {
            return Err(ApiError::InvalidCredentials(
                "Only a sent quotation can be accepted.".to_string(),
            ));
        }

        let approval_id = gate_price_locked(
            &mut db,
            quotation.plot_id,
            quotation.customer_id,
            quotation.payment_mode,
            quotation.quoted_price,
            Some(id),
        )?;

        let sale = execute_sale_locked(
            &mut db,
            quotation.plot_id,
            quotation.customer_id,
            quotation.payment_mode,
            quotation.quoted_price,
        )?;

        if let Some(approval_id) = approval_id {
            if let Some(req) = db.approval_requests.iter_mut().find(|r| r.id == approval_id) {
                req.resulting_sale_id = Some(sale.id);
            }
        }

        let quotation = db.quotations.iter_mut().find(|q| q.id == id).unwrap();
        quotation.status = QuotationStatus::Accepted;
        quotation.converted_sale_id = Some(sale.id);
        quotation.updated_at = Utc::now();
        Ok(quotation.clone())
    }

    pub async fn list_approvals(&self, status: Option<&str>) -> Result<Vec<ApprovalRequestSummary>, ApiError> {
        settle(150).await;
        let db = self.db.lock().unwrap();
        let mut out: Vec<ApprovalRequestSummary> = db
            .approval_requests
            .iter()
            .filter(|r| match status {
                Some(s) => to_pg_str(r.status) == s,
                None => true,
            })
            .filter_map(|r| approval_summary(&db, r))
            .collect();
        out.sort_by(|a, b| b.request.created_at.cmp(&a.request.created_at));
        Ok(out)
    }

    pub async fn approve_request(
        &self,
        id: Uuid,
        notes: Option<String>,
    ) -> Result<ApprovalRequestSummary, ApiError> {
        settle(200).await;
        self.decide_request(id, ApprovalStatus::Approved, notes)
    }

    pub async fn reject_request(
        &self,
        id: Uuid,
        notes: Option<String>,
    ) -> Result<ApprovalRequestSummary, ApiError> {
        settle(200).await;
        self.decide_request(id, ApprovalStatus::Rejected, notes)
    }

    fn decide_request(
        &self,
        id: Uuid,
        to: ApprovalStatus,
        notes: Option<String>,
    ) -> Result<ApprovalRequestSummary, ApiError> {
        let mut db = self.db.lock().unwrap();
        let requested_by = {
            let req = db
                .approval_requests
                .iter()
                .find(|r| r.id == id)
                .ok_or(ApiError::NotFound)?;
            if req.status != ApprovalStatus::Pending {
                return Err(ApiError::InvalidCredentials(
                    "This request has already been decided.".to_string(),
                ));
            }
            req.requested_by
        };

        // Mirrors `crates/backend/src/routes/approvals.rs::decide`'s
        // fallback: the mock only ever has one user, so unconditionally
        // blocking self-decision would make every gated sale
        // permanently stuck in this demo.
        let other_users_exist = false;
        if requested_by == db.demo_user.id && other_users_exist {
            return Err(ApiError::InvalidCredentials(
                "You can't decide on a request you submitted yourself.".to_string(),
            ));
        }

        let decided_by = db.demo_user.id;
        let req = db.approval_requests.iter_mut().find(|r| r.id == id).unwrap();
        req.status = to;
        req.decided_by = Some(decided_by);
        req.decided_at = Some(Utc::now());
        req.decision_notes = notes.filter(|s| !s.trim().is_empty());

        approval_summary(&db, db.approval_requests.iter().find(|r| r.id == id).unwrap())
            .ok_or(ApiError::NotFound)
    }

    pub async fn sales_report(
        &self,
        project_id: Option<Uuid>,
        agent_id: Option<Uuid>,
        from: Option<NaiveDate>,
        to: Option<NaiveDate>,
    ) -> Result<SalesReport, ApiError> {
        settle(150).await;
        let db = self.db.lock().unwrap();

        let mut rows: Vec<SalesReportRow> = db
            .sales
            .iter()
            .filter_map(|sale| {
                let plot = db.plots.iter().find(|p| p.id == sale.plot_id)?;
                let project = db.projects.iter().find(|p| p.id == plot.project_id)?;
                let customer = db.customers.iter().find(|c| c.id == sale.customer_id)?;
                let date = sale.created_at.date_naive();
                if project_id.is_some_and(|id| id != project.id) {
                    return None;
                }
                if agent_id.is_some_and(|id| Some(id) != sale.agent_id) {
                    return None;
                }
                if from.is_some_and(|from| date < from) {
                    return None;
                }
                if to.is_some_and(|to| date > to) {
                    return None;
                }
                Some(SalesReportRow {
                    sale_id: sale.id,
                    created_at: sale.created_at,
                    project_id: project.id,
                    project_name: project.name.clone(),
                    plot_number: plot.plot_number.clone(),
                    customer_id: customer.id,
                    customer_name: customer.full_name.clone(),
                    agent_id: sale.agent_id,
                    agent_name: sale.agent_id.map(|_| db.demo_user.full_name.clone()),
                    payment_mode: sale.payment_mode,
                    agreed_price: sale.agreed_price,
                })
            })
            .collect();
        rows.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        let total_count = rows.len() as u32;
        let total_value = rows.iter().map(|r| r.agreed_price).sum();
        Ok(SalesReport { rows, total_count, total_value })
    }

    pub async fn inventory_report(&self) -> Result<InventoryReport, ApiError> {
        settle(150).await;
        let db = self.db.lock().unwrap();

        let mut by_project: Vec<ProjectInventoryRow> = Vec::new();
        for plot in &db.plots {
            let Some(project) = db.projects.iter().find(|p| p.id == plot.project_id) else {
                continue;
            };
            let (label, color) = status_meta(plot.status);
            let idx = match by_project.iter().position(|p| p.project_id == project.id) {
                Some(idx) => idx,
                None => {
                    by_project.push(ProjectInventoryRow {
                        project_id: project.id,
                        project_name: project.name.clone(),
                        total_plots: 0,
                        by_status: Vec::new(),
                    });
                    by_project.len() - 1
                }
            };
            let row = &mut by_project[idx];
            row.total_plots += 1;
            match row.by_status.iter_mut().find(|s| s.status == plot.status) {
                Some(entry) => {
                    entry.count += 1;
                    entry.value += plot.asking_price;
                }
                None => row.by_status.push(PlotStatusCount {
                    status: plot.status,
                    status_label: label.to_string(),
                    status_color: color.to_string(),
                    count: 1,
                    value: plot.asking_price,
                }),
            }
        }
        by_project.sort_by(|a, b| a.project_name.cmp(&b.project_name));

        Ok(InventoryReport { by_project })
    }

    pub async fn agent_performance_report(
        &self,
        from: Option<NaiveDate>,
        to: Option<NaiveDate>,
    ) -> Result<AgentPerformanceReport, ApiError> {
        settle(150).await;
        let db = self.db.lock().unwrap();

        // The mock only ever has one user (`demo_user`) to attribute
        // sales/quotations to — see the same note on `decide_request`.
        let in_range = |date: NaiveDate| {
            !from.is_some_and(|from| date < from) && !to.is_some_and(|to| date > to)
        };
        let sales_count = db.sales.iter().filter(|s| in_range(s.created_at.date_naive())).count() as u32;
        let sales_value: Decimal = db
            .sales
            .iter()
            .filter(|s| in_range(s.created_at.date_naive()))
            .map(|s| s.agreed_price)
            .sum();
        let quotations_sent = db
            .quotations
            .iter()
            .filter(|q| in_range(q.created_at.date_naive()))
            .filter(|q| q.status != QuotationStatus::Draft)
            .count() as u32;
        let quotations_accepted = db
            .quotations
            .iter()
            .filter(|q| in_range(q.created_at.date_naive()))
            .filter(|q| q.status == QuotationStatus::Accepted)
            .count() as u32;

        let rows = if sales_count == 0 && quotations_sent == 0 {
            Vec::new()
        } else {
            vec![AgentPerformanceRow {
                agent_id: db.demo_user.id,
                agent_name: db.demo_user.full_name.clone(),
                sales_count,
                sales_value,
                quotations_sent,
                quotations_accepted,
            }]
        };

        Ok(AgentPerformanceReport { rows })
    }

    pub async fn get_map_summary(&self, project_id: Uuid) -> Result<ProjectMapSummary, ApiError> {
        settle(100).await;
        let db = self.db.lock().unwrap();
        Ok(match db.project_maps.get(&project_id) {
            Some(map) => ProjectMapSummary {
                exists: true,
                image_content_type: Some(map.image_content_type.clone()),
                polygons: map.polygons.clone(),
                updated_at: Some(map.updated_at),
            },
            None => ProjectMapSummary {
                exists: false,
                image_content_type: None,
                polygons: MapPolygons::default(),
                updated_at: None,
            },
        })
    }

    pub async fn upload_map_image(
        &self,
        project_id: Uuid,
        file: web_sys::File,
    ) -> Result<ProjectMapSummary, ApiError> {
        settle(300).await;
        let content_type = file.type_();
        let url = web_sys::Url::create_object_url_with_blob(&file)
            .map_err(|_| ApiError::Network("couldn't read that file".to_string()))?;

        let mut db = self.db.lock().unwrap();
        db.project_maps.insert(
            project_id,
            MockProjectMap {
                image_url: url,
                image_content_type: content_type,
                polygons: MapPolygons::default(),
                updated_at: Utc::now(),
            },
        );
        drop(db);
        self.get_map_summary(project_id).await
    }

    pub async fn update_map_polygons(
        &self,
        project_id: Uuid,
        polygons: MapPolygons,
    ) -> Result<ProjectMapSummary, ApiError> {
        settle(200).await;
        {
            let db = self.db.lock().unwrap();
            let mut seen_plot_ids = std::collections::HashSet::new();
            for feature in &polygons.features {
                let Some(plot_id) = feature.plot_id else {
                    continue;
                };
                if !seen_plot_ids.insert(plot_id) {
                    return Err(ApiError::InvalidCredentials(
                        "Each plot can only be linked to one shape on the map.".to_string(),
                    ));
                }
                let belongs = db
                    .plots
                    .iter()
                    .any(|p| p.id == plot_id && p.project_id == project_id);
                if !belongs {
                    return Err(ApiError::InvalidCredentials(format!(
                        "Plot {plot_id} doesn't belong to this project."
                    )));
                }
            }
        }

        let mut db = self.db.lock().unwrap();
        let Some(map) = db.project_maps.get_mut(&project_id) else {
            return Err(ApiError::InvalidCredentials(
                "Upload a site plan image before saving plot boundaries.".to_string(),
            ));
        };
        map.polygons = polygons;
        map.updated_at = Utc::now();
        drop(db);
        self.get_map_summary(project_id).await
    }

    pub async fn create_plot_for_map_feature(
        &self,
        project_id: Uuid,
        feature_id: &str,
        input: CreatePlotInput,
    ) -> Result<ProjectMapSummary, ApiError> {
        settle(300).await;
        let plot = self.create_plot(input).await?;
        let mut db = self.db.lock().unwrap();
        let Some(map) = db.project_maps.get_mut(&project_id) else {
            return Err(ApiError::NotFound);
        };
        let Some(feature) = map.polygons.features.iter_mut().find(|f| f.id == feature_id) else {
            return Err(ApiError::NotFound);
        };
        if feature.plot_id.is_some() {
            return Err(ApiError::InvalidCredentials(
                "This shape is already linked to a plot.".to_string(),
            ));
        }
        feature.plot_id = Some(plot.id);
        map.updated_at = Utc::now();
        drop(db);
        self.get_map_summary(project_id).await
    }

    pub async fn link_plot_to_map_feature(
        &self,
        project_id: Uuid,
        feature_id: &str,
        plot_id: Uuid,
    ) -> Result<ProjectMapSummary, ApiError> {
        settle(200).await;
        let mut db = self.db.lock().unwrap();
        let plot_ok = db.plots.iter().any(|p| p.id == plot_id && p.project_id == project_id);
        if !plot_ok {
            return Err(ApiError::InvalidCredentials(
                "That plot doesn't belong to this project.".to_string(),
            ));
        }
        let Some(map) = db.project_maps.get_mut(&project_id) else {
            return Err(ApiError::NotFound);
        };
        if map.polygons.features.iter().any(|f| f.plot_id == Some(plot_id)) {
            return Err(ApiError::InvalidCredentials(
                "That plot is already linked to a shape on this map.".to_string(),
            ));
        }
        let Some(feature) = map.polygons.features.iter_mut().find(|f| f.id == feature_id) else {
            return Err(ApiError::NotFound);
        };
        if feature.plot_id.is_some() {
            return Err(ApiError::InvalidCredentials(
                "This shape is already linked to a plot.".to_string(),
            ));
        }
        feature.plot_id = Some(plot_id);
        map.updated_at = Utc::now();
        drop(db);
        self.get_map_summary(project_id).await
    }

    pub async fn unlink_map_feature(
        &self,
        project_id: Uuid,
        feature_id: &str,
    ) -> Result<ProjectMapSummary, ApiError> {
        settle(200).await;
        let mut db = self.db.lock().unwrap();
        let Some(map) = db.project_maps.get_mut(&project_id) else {
            return Err(ApiError::NotFound);
        };
        let Some(feature) = map.polygons.features.iter_mut().find(|f| f.id == feature_id) else {
            return Err(ApiError::NotFound);
        };
        feature.plot_id = None;
        map.updated_at = Utc::now();
        drop(db);
        self.get_map_summary(project_id).await
    }

    pub fn map_image_url(&self, project_id: Uuid) -> String {
        self.db
            .lock()
            .unwrap()
            .project_maps
            .get(&project_id)
            .map(|m| m.image_url.clone())
            .unwrap_or_default()
    }

    /// Reuses `create_plot`/`create_customer` per row rather than a
    /// separate in-memory insert path — mirrors
    /// `crates/backend/src/routes/projects.rs`'s `insert_plot` /
    /// `routes/customers.rs`'s `insert_customer` being the single
    /// place both the one-row and bulk endpoints go through.
    pub async fn bulk_create_plots(
        &self,
        project_id: Uuid,
        inputs: Vec<CreatePlotInput>,
    ) -> Result<domain::BulkImportResult, ApiError> {
        let mut created = 0u32;
        let mut errors = Vec::new();
        for (idx, mut input) in inputs.into_iter().enumerate() {
            input.project_id = project_id;
            match self.create_plot(input).await {
                Ok(_) => created += 1,
                Err(ApiError::InvalidCredentials(message)) => {
                    errors.push(domain::BulkImportRowError { row: idx as u32 + 1, message })
                }
                Err(e) => errors.push(domain::BulkImportRowError {
                    row: idx as u32 + 1,
                    message: format!("{e}"),
                }),
            }
        }
        Ok(domain::BulkImportResult { created, errors })
    }

    pub async fn bulk_create_customers(
        &self,
        inputs: Vec<CreateCustomerInput>,
    ) -> Result<domain::BulkImportResult, ApiError> {
        let mut created = 0u32;
        let mut errors = Vec::new();
        for (idx, input) in inputs.into_iter().enumerate() {
            match self.create_customer(input).await {
                Ok(_) => created += 1,
                Err(ApiError::InvalidCredentials(message)) => {
                    errors.push(domain::BulkImportRowError { row: idx as u32 + 1, message })
                }
                Err(e) => errors.push(domain::BulkImportRowError {
                    row: idx as u32 + 1,
                    message: format!("{e}"),
                }),
            }
        }
        Ok(domain::BulkImportResult { created, errors })
    }

    /// Mirrors `crates/backend/src/routes/sales.rs::bulk_create_sales`
    /// — see `domain::BulkSaleRow`'s module docs for why this carries
    /// `amount_paid` instead of starting every imported Lipa Pole
    /// Pole account at zero like `execute_sale_locked` does for a
    /// brand-new sale.
    pub async fn bulk_create_sales(
        &self,
        inputs: Vec<BulkSaleRow>,
    ) -> Result<domain::BulkImportResult, ApiError> {
        let mut db = self.db.lock().unwrap();
        let mut created = 0u32;
        let mut errors = Vec::new();
        for (idx, input) in inputs.iter().enumerate() {
            match insert_bulk_sale_locked(&mut db, input) {
                Ok(_) => created += 1,
                Err(ApiError::InvalidCredentials(message)) => {
                    errors.push(domain::BulkImportRowError { row: idx as u32 + 1, message })
                }
                Err(e) => errors.push(domain::BulkImportRowError {
                    row: idx as u32 + 1,
                    message: format!("{e}"),
                }),
            }
        }
        Ok(domain::BulkImportResult { created, errors })
    }

    pub async fn get_settings(&self) -> Result<domain::OrganizationSettings, ApiError> {
        settle(150).await;
        let db = self.db.lock().unwrap();
        Ok(build_settings(&db))
    }

    pub async fn update_settings(
        &self,
        input: domain::UpdateOrganizationSettingsInput,
    ) -> Result<domain::OrganizationSettings, ApiError> {
        settle(200).await;
        let currency = input.currency.trim().to_uppercase();
        if currency.len() < 2 || currency.len() > 5 || !currency.chars().all(|c| c.is_ascii_alphabetic()) {
            return Err(ApiError::InvalidCredentials(
                "Currency must be a 2-5 letter code, e.g. KES or USD.".to_string(),
            ));
        }
        for cfg in [&input.plot_numbering, &input.project_numbering] {
            if !(1..=10).contains(&cfg.padding) {
                return Err(ApiError::InvalidCredentials(
                    "Numbering digit padding must be between 1 and 10.".to_string(),
                ));
            }
            if cfg.next_number == 0 {
                return Err(ApiError::InvalidCredentials(
                    "The next number to issue must be at least 1.".to_string(),
                ));
            }
        }

        let mut db = self.db.lock().unwrap();
        db.organization.currency = currency;
        db.date_format = input.date_format.trim().to_string();
        db.timezone = input.timezone.trim().to_string();
        db.plot_numbering = MockNumbering {
            prefix: input.plot_numbering.prefix.trim().to_string(),
            include_year: input.plot_numbering.include_year,
            include_entity_code: input.plot_numbering.include_entity_code,
            padding: input.plot_numbering.padding,
            next_number: input.plot_numbering.next_number,
        };
        db.project_numbering = MockNumbering {
            prefix: input.project_numbering.prefix.trim().to_string(),
            include_year: input.project_numbering.include_year,
            include_entity_code: input.project_numbering.include_entity_code,
            padding: input.project_numbering.padding,
            next_number: input.project_numbering.next_number,
        };
        Ok(build_settings(&db))
    }

    pub async fn next_number(
        &self,
        entity_type: &str,
        project_code: Option<&str>,
    ) -> Result<String, ApiError> {
        settle(150).await;
        let mut db = self.db.lock().unwrap();
        let cfg = match entity_type {
            "plot" => &mut db.plot_numbering,
            "project" => &mut db.project_numbering,
            _ => return Err(ApiError::NotFound),
        };
        let issued = cfg.next_number;
        cfg.next_number += 1;
        let entity_code = if entity_type == "plot" { project_code } else { None };
        Ok(domain::format_sequence_number(
            &cfg.prefix,
            cfg.include_year,
            entity_code,
            cfg.padding,
            issued,
        ))
    }
}

/// Shared by `get_settings`/`update_settings` — builds the wire payload,
/// including each numbering config's live preview, from `MockDb`'s
/// current state. The `db` lock must already be held by the caller.
fn build_settings(db: &MockDb) -> domain::OrganizationSettings {
    let numbering_config = |entity_type: domain::NumberingEntityType, cfg: &MockNumbering| {
        let placeholder_code = (entity_type == domain::NumberingEntityType::Plot
            && cfg.include_entity_code)
            .then_some("ABC");
        domain::NumberingConfig {
            entity_type,
            prefix: cfg.prefix.clone(),
            include_year: cfg.include_year,
            include_entity_code: cfg.include_entity_code,
            padding: cfg.padding,
            next_number: cfg.next_number,
            preview: domain::format_sequence_number(
                &cfg.prefix,
                cfg.include_year,
                placeholder_code,
                cfg.padding,
                cfg.next_number,
            ),
        }
    };
    domain::OrganizationSettings {
        organization_id: db.organization.id,
        name: db.organization.name.clone(),
        currency: db.organization.currency.clone(),
        date_format: db.date_format.clone(),
        timezone: db.timezone.clone(),
        plot_numbering: numbering_config(domain::NumberingEntityType::Plot, &db.plot_numbering),
        project_numbering: numbering_config(
            domain::NumberingEntityType::Project,
            &db.project_numbering,
        ),
    }
}

fn validate_dimensions_mock(side_1: Option<Decimal>, side_2: Option<Decimal>) -> Result<(), ApiError> {
    if side_1.is_some_and(|v| v <= Decimal::ZERO) || side_2.is_some_and(|v| v <= Decimal::ZERO) {
        return Err(ApiError::InvalidCredentials(
            "Plot dimensions must be greater than zero.".to_string(),
        ));
    }
    Ok(())
}

fn to_pg_str(status: ApprovalStatus) -> &'static str {
    match status {
        ApprovalStatus::Pending => "pending",
        ApprovalStatus::Approved => "approved",
        ApprovalStatus::Rejected => "rejected",
    }
}

fn approval_summary(db: &MockDb, r: &ApprovalRequest) -> Option<ApprovalRequestSummary> {
    let plot = db.plots.iter().find(|p| p.id == r.plot_id)?;
    let project = db.projects.iter().find(|p| p.id == plot.project_id)?;
    let customer = db.customers.iter().find(|c| c.id == r.customer_id)?;
    let (label, color) = approval_status_meta(r.status);
    Some(ApprovalRequestSummary {
        request: r.clone(),
        plot_number: plot.plot_number.clone(),
        project_name: project.name.clone(),
        customer_name: customer.full_name.clone(),
        requested_by_name: db.demo_user.full_name.clone(),
        decided_by_name: r.decided_by.map(|_| db.demo_user.full_name.clone()),
        status_label: label.to_string(),
        status_color: color.to_string(),
    })
}

/// Mirrors `crates/backend/src/routes/approvals.rs::gate_price` — see
/// its docs for the full contract (`Ok(None)` = no gate needed,
/// `Ok(Some(id))` = an approved request should be consumed by the
/// caller, `Err` = now pending). No transaction-boundary subtlety here
/// like the real one has to worry about (recording a pending request
/// must survive even though the sale doesn't happen) — `MockDb` isn't
/// transactional, a `Vec::push` just stays pushed.
fn gate_price_locked(
    db: &mut MockDb,
    plot_id: Uuid,
    customer_id: Uuid,
    payment_mode: PaymentMode,
    agreed_price: Decimal,
    quotation_id: Option<Uuid>,
) -> Result<Option<Uuid>, ApiError> {
    let minimum_price = db
        .plots
        .iter()
        .find(|p| p.id == plot_id)
        .ok_or(ApiError::NotFound)?
        .minimum_price;

    if agreed_price >= minimum_price {
        return Ok(None);
    }

    let approved = db
        .approval_requests
        .iter()
        .find(|r| {
            r.plot_id == plot_id
                && r.customer_id == customer_id
                && r.payment_mode == payment_mode
                && r.agreed_price == agreed_price
                && r.status == ApprovalStatus::Approved
                && r.resulting_sale_id.is_none()
        })
        .map(|r| r.id);
    if let Some(id) = approved {
        return Ok(Some(id));
    }

    let already_pending = db.approval_requests.iter().any(|r| {
        r.plot_id == plot_id
            && r.customer_id == customer_id
            && r.payment_mode == payment_mode
            && r.agreed_price == agreed_price
            && r.status == ApprovalStatus::Pending
    });
    if already_pending {
        return Err(ApiError::InvalidCredentials(
            "This price is below the plot's minimum and is still awaiting approval.".to_string(),
        ));
    }

    let organization_id = db.organization.id;
    let requested_by = db.demo_user.id;
    db.approval_requests.push(ApprovalRequest {
        id: Uuid::new_v4(),
        organization_id,
        plot_id,
        customer_id,
        agent_id: Some(requested_by),
        payment_mode,
        agreed_price,
        minimum_price,
        quotation_id,
        requested_by,
        reason: format!("Price {agreed_price} is below this plot's minimum of {minimum_price}."),
        status: ApprovalStatus::Pending,
        decided_by: None,
        decided_at: None,
        decision_notes: None,
        resulting_sale_id: None,
        created_at: Utc::now(),
    });

    Err(ApiError::InvalidCredentials(
        "This price is below the plot's minimum. A request has been sent for approval — try again once it's approved.".to_string(),
    ))
}

fn quotation_is_expired(q: &Quotation) -> bool {
    q.status == QuotationStatus::Sent && q.valid_until < Utc::now().date_naive()
}

fn quotation_summary(db: &MockDb, q: &Quotation) -> Option<QuotationSummary> {
    let plot = db.plots.iter().find(|p| p.id == q.plot_id)?;
    let project = db.projects.iter().find(|p| p.id == plot.project_id)?;
    let customer = db.customers.iter().find(|c| c.id == q.customer_id)?;
    let expired = quotation_is_expired(q);
    let (label, color) = quotation_status_meta(q.status, expired);
    Some(QuotationSummary {
        quotation: q.clone(),
        plot_number: plot.plot_number.clone(),
        project_name: project.name.clone(),
        customer_name: customer.full_name.clone(),
        status_label: label.to_string(),
        status_color: color.to_string(),
        is_expired: expired,
    })
}

fn quotation_detail(db: &MockDb, q: &Quotation) -> Option<QuotationDetail> {
    let plot = db.plots.iter().find(|p| p.id == q.plot_id)?;
    let project = db.projects.iter().find(|p| p.id == plot.project_id)?;
    let customer = db.customers.iter().find(|c| c.id == q.customer_id)?;
    let expired = quotation_is_expired(q);
    let (label, color) = quotation_status_meta(q.status, expired);
    Some(QuotationDetail {
        quotation: q.clone(),
        plot_id: plot.id,
        plot_number: plot.plot_number.clone(),
        project_id: project.id,
        project_name: project.name.clone(),
        asking_price: plot.asking_price,
        minimum_price: plot.minimum_price,
        customer_id: customer.id,
        customer_name: customer.full_name.clone(),
        status_label: label.to_string(),
        status_color: color.to_string(),
        is_expired: expired,
        below_minimum_price: q.quoted_price < plot.minimum_price,
    })
}

/// The actual "commit to a sale" logic, taking the already-locked
/// `MockDb` — a plain function rather than a `&self` method because
/// `accept_quotation` needs to run it from inside its own already-held
/// lock (the `Mutex` isn't reentrant, so calling `self.create_sale()`
/// from there would deadlock). Mirrors
/// `crates/backend/src/routes/sales.rs`'s `execute_sale` split for
/// exactly the same reason: one place both `create_sale` and quotation
/// acceptance go through, so they can't drift.
fn execute_sale_locked(
    db: &mut MockDb,
    plot_id: Uuid,
    customer_id: Uuid,
    payment_mode: PaymentMode,
    agreed_price: Decimal,
) -> Result<PlotSale, ApiError> {
    if !db.customers.iter().any(|c| c.id == customer_id) {
        return Err(ApiError::NotFound);
    }

    let already_sold = db.sales.iter().any(|s| s.plot_id == plot_id);
    if already_sold {
        return Err(ApiError::InvalidCredentials(
            "This plot already has an active sale.".to_string(),
        ));
    }

    let organization_id = db.organization.id;
    let agent_id = db.demo_user.id;
    let sale = PlotSale {
        id: Uuid::new_v4(),
        plot_id,
        customer_id,
        organization_id,
        agent_id: Some(agent_id),
        payment_mode,
        agreed_price,
        created_at: Utc::now(),
    };
    db.sales.push(sale.clone());

    if payment_mode != PaymentMode::FullCash {
        let seq = db.loan_accounts.len() + 1;
        let today = Utc::now().date_naive();
        db.loan_accounts.push(new_loan_account(&sale, seq, today));
    }

    if let Some(plot) = db.plots.iter_mut().find(|p| p.id == plot_id) {
        plot.status = match payment_mode {
            PaymentMode::FullCash => PlotStatus::Reserved,
            PaymentMode::LipaPolePoleInterestFree | PaymentMode::LipaPolePoleInterestBearing => {
                PlotStatus::Booked
            }
        };
        plot.assigned_customer_id = Some(customer_id);
    }

    Ok(sale)
}

/// Mirrors `crates/backend/src/routes/sales.rs::insert_bulk_sale` —
/// its own function rather than reusing `execute_sale_locked`, since
/// a historical import needs to set `amount_paid`/status/plot status
/// from what's already been repaid instead of always starting at
/// zero like a fresh sale does.
fn insert_bulk_sale_locked(db: &mut MockDb, input: &BulkSaleRow) -> Result<(), ApiError> {
    if input.agreed_price <= Decimal::ZERO {
        return Err(ApiError::InvalidCredentials(
            "Enter an agreed price greater than zero.".to_string(),
        ));
    }
    let amount_paid = input.amount_paid.max(Decimal::ZERO).min(input.agreed_price);

    let plot_id = db
        .projects
        .iter()
        .find(|p| p.code == input.project_code)
        .and_then(|p| db.plots.iter().find(|pl| pl.project_id == p.id && pl.plot_number == input.plot_number))
        .map(|pl| pl.id)
        .ok_or_else(|| {
            ApiError::InvalidCredentials(format!(
                "No plot \"{}\" found in project \"{}\".",
                input.plot_number, input.project_code
            ))
        })?;

    let lookup = input.customer_lookup.trim();
    if lookup.is_empty() {
        return Err(ApiError::InvalidCredentials(
            "Provide a customer ID number, phone, or email to match an existing customer."
                .to_string(),
        ));
    }
    let customer_id = db
        .customers
        .iter()
        .find(|c| {
            c.id_number.as_deref() == Some(lookup)
                || c.phone.as_deref() == Some(lookup)
                || c.email.as_deref() == Some(lookup)
        })
        .map(|c| c.id)
        .ok_or_else(|| {
            ApiError::InvalidCredentials(format!(
                "No existing customer matches \"{lookup}\" — import customers first."
            ))
        })?;

    if db.sales.iter().any(|s| s.plot_id == plot_id) {
        return Err(ApiError::InvalidCredentials(
            "This plot already has an active sale.".to_string(),
        ));
    }

    let organization_id = db.organization.id;
    let sale = PlotSale {
        id: Uuid::new_v4(),
        plot_id,
        customer_id,
        organization_id,
        agent_id: None,
        payment_mode: input.payment_mode,
        agreed_price: input.agreed_price,
        created_at: input.sale_date.and_hms_opt(12, 0, 0).unwrap().and_utc(),
    };

    let fully_paid = input.payment_mode == PaymentMode::FullCash || amount_paid >= input.agreed_price;

    if input.payment_mode != PaymentMode::FullCash {
        let seq = db.loan_accounts.len() + 1;
        let mut account = new_loan_account(&sale, seq, input.sale_date);
        account.deposit_paid = amount_paid.min(account.deposit_required);
        account.amount_paid = amount_paid;
        account.outstanding_balance = input.agreed_price - amount_paid;
        account.status = if fully_paid {
            LoanAccountStatus::FullyPaid
        } else if amount_paid > Decimal::ZERO {
            LoanAccountStatus::ActivePartiallyPaid
        } else {
            LoanAccountStatus::ApprovedAwaitingDeposit
        };
        db.loan_accounts.push(account);
    }

    if let Some(plot) = db.plots.iter_mut().find(|p| p.id == plot_id) {
        plot.status = if fully_paid { PlotStatus::Sold } else { PlotStatus::Booked };
        plot.assigned_customer_id = Some(customer_id);
    }

    db.sales.push(sale);
    Ok(())
}

/// Deliberately simple defaults (10% deposit, 12 monthly instalments) —
/// see `CreateSaleInput`'s doc comment. Shared by `create_sale` and the
/// initial seed so both produce accounts with the same shape.
fn new_loan_account(sale: &PlotSale, seq: usize, start_date: NaiveDate) -> PlotLoanAccount {
    let deposit_required = (sale.agreed_price * Decimal::new(10, 2)).round();
    let financed = sale.agreed_price - deposit_required;
    let instalment_amount = (financed / Decimal::from(12)).round();
    let interest_rate = match sale.payment_mode {
        PaymentMode::LipaPolePoleInterestBearing => Some(Decimal::from(14)),
        _ => None,
    };

    PlotLoanAccount {
        id: Uuid::new_v4(),
        account_number: format!("PLA-{seq:04}"),
        sale_id: sale.id,
        principal: sale.agreed_price,
        interest_rate,
        deposit_required,
        deposit_paid: Decimal::ZERO,
        instalment_amount,
        repayment_frequency_days: 30,
        start_date,
        status: LoanAccountStatus::ApprovedAwaitingDeposit,
        amount_paid: Decimal::ZERO,
        outstanding_balance: sale.agreed_price,
        days_in_arrears: 0,
    }
}

async fn settle(millis: u32) {
    gloo_timers::future::TimeoutFuture::new(millis).await;
}

fn seed() -> MockDb {
    let org_id = Uuid::new_v4();
    let organization = Organization {
        id: org_id,
        name: "Acacia Grove Properties".to_string(),
        code: "ACACIA".to_string(),
        currency: "KES".to_string(),
        created_at: Utc::now(),
    };
    let date_format = "DD/MM/YYYY".to_string();
    let timezone = "Africa/Nairobi".to_string();
    let plot_numbering = MockNumbering {
        prefix: "PLT".to_string(),
        include_year: false,
        include_entity_code: false,
        padding: 4,
        next_number: 1,
    };
    let project_numbering = MockNumbering {
        prefix: "PRJ".to_string(),
        include_year: false,
        include_entity_code: false,
        padding: 4,
        next_number: 1,
    };

    let demo_user = User {
        id: Uuid::new_v4(),
        organization_id: organization.id,
        branch_id: None,
        full_name: "Amina Wanjiru".to_string(),
        email: DEMO_EMAIL.to_string(),
        is_active: true,
        is_platform_owner: false,
        must_change_password: false,
        created_at: Utc::now(),
    };
    let admin_role_id = Uuid::new_v4();

    let project_specs = [
        ("Acacia Grove — Phase I", "AG-P1", "Kitengela, Kajiado", 16),
        ("Riverside Meadows", "RM", "Malaa, Machakos", 12),
        ("Sunview Gardens", "SVG", "Kangundo Road, Machakos", 10),
    ];

    let statuses = [
        PlotStatus::Available,
        PlotStatus::Available,
        PlotStatus::Available,
        PlotStatus::Selected,
        PlotStatus::TemporarilyHeld,
        PlotStatus::Reserved,
        PlotStatus::Booked,
        PlotStatus::UnderApproval,
        PlotStatus::Sold,
        PlotStatus::TransferInProgress,
        PlotStatus::Transferred,
        PlotStatus::Blocked,
        PlotStatus::Disputed,
        PlotStatus::Cancelled,
    ];

    let mut projects = Vec::new();
    let mut plots = Vec::new();

    for (name, code, location, plot_count) in project_specs {
        let project_id = Uuid::new_v4();
        projects.push(Project {
            id: project_id,
            organization_id: organization.id,
            branch_id: None,
            name: name.to_string(),
            code: code.to_string(),
            location: location.to_string(),
            original_title_number: Some(format!("{code}/TITLE/0042")),
            total_size: Decimal::from(plot_count) * Decimal::new(125, 2), // ~1.25 acres/plot
            area_unit: AreaUnit::Acres,
            status: ProjectStatus::Active,
            assigned_manager_id: Some(demo_user.id),
            created_at: Utc::now(),
        });

        for n in 1..=plot_count {
            let status = statuses[(n as usize - 1) % statuses.len()];
            let base_price = Decimal::from(650_000 + (n as i64 % 5) * 35_000);
            // Half the demo plots carry recorded dimensions, half don't —
            // exercises both the "80 × 100 ft" and "Not specified" display
            // paths without a separate fixture.
            let has_dimensions = n % 2 == 1;
            plots.push(Plot {
                id: Uuid::new_v4(),
                project_id,
                plot_number: format!("{code}-{n:03}"),
                title_number: matches!(status, PlotStatus::Sold | PlotStatus::Transferred)
                    .then(|| format!("{code}/TITLE/{n:04}")),
                size: Decimal::new(125, 2),
                side_1: has_dimensions.then(|| Decimal::from(50)),
                side_2: has_dimensions.then(|| Decimal::from(109)),
                dimension_unit: "ft".to_string(),
                asking_price: base_price,
                minimum_price: base_price - Decimal::from(50_000),
                status,
                map_feature_id: None,
                assigned_customer_id: None,
                created_at: Utc::now(),
            });
        }
    }

    let customers = vec![
        Customer {
            id: Uuid::new_v4(),
            organization_id: organization.id,
            full_name: "James Otieno".to_string(),
            email: Some("j.otieno@example.com".to_string()),
            phone: Some("0722 000 111".to_string()),
            id_number: Some("29889001".to_string()),
            assigned_agent_id: Some(demo_user.id),
            stage: LeadStage::New,
            source: Some("Referral".to_string()),
            next_follow_up_at: None,
            notes: None,
            created_at: Utc::now(),
        },
        Customer {
            id: Uuid::new_v4(),
            organization_id: organization.id,
            full_name: "Grace Mumbi".to_string(),
            email: Some("grace.mumbi@example.com".to_string()),
            phone: Some("0733 222 444".to_string()),
            id_number: Some("30112233".to_string()),
            assigned_agent_id: Some(demo_user.id),
            stage: LeadStage::New,
            source: Some("Walk-in".to_string()),
            next_follow_up_at: None,
            notes: None,
            created_at: Utc::now(),
        },
        Customer {
            id: Uuid::new_v4(),
            organization_id: organization.id,
            full_name: "Peter Kariuki".to_string(),
            email: Some("p.kariuki@example.com".to_string()),
            phone: Some("0711 555 222".to_string()),
            id_number: None,
            assigned_agent_id: Some(demo_user.id),
            stage: LeadStage::SiteVisit,
            source: Some("Website".to_string()),
            next_follow_up_at: Some(Utc::now().date_naive() + chrono::Duration::days(3)),
            notes: Some("Interested in a corner plot at Riverside Meadows, budget ~1.2M.".to_string()),
            created_at: Utc::now(),
        },
    ];

    // Plots at "Reserved" or beyond represent a real booking/sale in
    // progress (docs/05's status table — Selected/Temporarily Held are
    // pre-sale interest, not yet a sale record) — give each one a
    // matching PlotSale and assign it to one of the seeded customers, so
    // the customer-detail screen and the "already sold" guard in
    // `create_sale` have something real to show/enforce.
    let mut sales = Vec::new();
    let mut loan_accounts = Vec::new();
    let mut payments = Vec::new();
    let today = Utc::now().date_naive();
    let mut next_customer = 0usize;
    // Only the first two seeded customers (James, Grace) get a sale —
    // Peter stays a pure lead with no plots_owned, so the Leads view has
    // something real to show instead of an always-empty demo.
    let converted_customer_pool = 2.min(customers.len());
    for plot in plots.iter_mut() {
        let payment_mode = match plot.status {
            PlotStatus::Reserved | PlotStatus::Sold | PlotStatus::Transferred => {
                PaymentMode::FullCash
            }
            PlotStatus::Booked
            | PlotStatus::UnderApproval
            | PlotStatus::TransferInProgress => PaymentMode::LipaPolePoleInterestFree,
            _ => continue,
        };
        let customer = &customers[next_customer % converted_customer_pool];
        next_customer += 1;

        plot.assigned_customer_id = Some(customer.id);
        // Spread across the trailing ~10 months (not all `Utc::now()`)
        // so the executive dashboard's monthly trend chart has an
        // actual trend to show in the mock/demo instead of one spike
        // in the current month and eleven empty ones.
        let months_back = (next_customer % 10) as i64;
        let sale_created_at =
            Utc::now() - chrono::Duration::days(30 * months_back + (next_customer as i64 % 7));
        let sale = PlotSale {
            id: Uuid::new_v4(),
            plot_id: plot.id,
            customer_id: customer.id,
            organization_id: organization.id,
            agent_id: Some(demo_user.id),
            payment_mode,
            agreed_price: plot.asking_price,
            created_at: sale_created_at,
        };

        if payment_mode != PaymentMode::FullCash {
            let mut account = new_loan_account(&sale, loan_accounts.len() + 1, today);
            // Give it a couple of instalments of history so the loan
            // account detail screen has something real to show, instead
            // of every seeded account looking freshly opened.
            let paid = account.instalment_amount * Decimal::from(2);
            account.amount_paid = paid;
            account.outstanding_balance = (account.principal - paid).max(Decimal::ZERO);
            account.status = LoanAccountStatus::ActivePartiallyPaid;
            payments.push(Payment {
                id: Uuid::new_v4(),
                loan_account_id: account.id,
                amount: account.instalment_amount,
                payment_date: today - chrono::Duration::days(60),
                method: "M-Pesa".to_string(),
                external_reference: Some("QGX7T2K9".to_string()),
                status: PaymentStatus::Posted,
                captured_by: demo_user.id,
                verified_by: Some(demo_user.id),
                created_at: Utc::now(),
            });
            payments.push(Payment {
                id: Uuid::new_v4(),
                loan_account_id: account.id,
                amount: account.instalment_amount,
                payment_date: today - chrono::Duration::days(30),
                method: "Bank Transfer".to_string(),
                external_reference: Some("FT2409".to_string()),
                status: PaymentStatus::Posted,
                captured_by: demo_user.id,
                verified_by: Some(demo_user.id),
                created_at: Utc::now(),
            });
            loan_accounts.push(account);
        }

        sales.push(sale);
    }

    let demo_tenant_user = domain::TenantUser {
        id: demo_user.id,
        full_name: demo_user.full_name.clone(),
        email: demo_user.email.clone(),
        mobile: None,
        is_active: true,
        is_platform_owner: false,
        branch_id: None,
        branch_name: None,
        branch_ids: Vec::new(),
        branch_count: 0,
        role_id: Some(admin_role_id),
        role_name: Some("Admin".to_string()),
        last_login_at: None,
        created_at: demo_user.created_at,
        must_change_password: false,
        password_changed_at: demo_user.created_at,
    };

    MockDb {
        organization,
        date_format,
        timezone,
        plot_numbering,
        project_numbering,
        demo_user,
        projects,
        plots,
        customers,
        sales,
        loan_accounts,
        payments,
        quotations: Vec::new(),
        approval_requests: Vec::new(),
        project_maps: HashMap::new(),
        roles: vec![domain::Role {
            id: admin_role_id,
            organization_id: org_id,
            name: "Admin".to_string(),
            permissions: vec!["*".to_string()],
            assigned_user_count: 1,
        }],
        tenant_users: vec![demo_tenant_user],
        // No branches exist yet in the real backend either (`branches`
        // has never had a row inserted — Settings -> Branches, a later
        // phase, is what creates the first one), so the mock matches
        // that reality rather than pre-seeding one.
        branches: Vec::new(),
    }
}
