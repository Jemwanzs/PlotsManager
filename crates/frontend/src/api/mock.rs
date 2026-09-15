//! In-memory sample data standing in for the real Rust API while the UI
//! is built ahead of it (see docs/14-development-roadmap.md). Every
//! method here has the exact signature `api::http::HttpApi` will
//! eventually have, so swapping `ApiClient::new_mock()` for
//! `ApiClient::new_http(base_url)` at the one call site in `app.rs` is
//! the entire migration — no component touches this module directly.

use std::sync::{Arc, Mutex};

use chrono::{NaiveDate, Utc};
use domain::{
    loan_status_meta, plot_status_meta as status_meta, AreaUnit, ApiError, AuthSession,
    CreateCustomerInput, CreatePlotInput, CreateProjectInput, CreateSaleInput, Customer,
    CustomerDetail, CustomerSaleView, CustomerSummary, DashboardSummary, LoanAccountDetail,
    LoanAccountStatus, Organization, Payment, PaymentMode, PaymentStatus, Plot, PlotLoanAccount,
    PlotSale, PlotStatus, PlotWithColor, Project, ProjectStatus, ProjectSummary,
    RecordPaymentInput, User,
};
use rust_decimal::Decimal;
use uuid::Uuid;

const DEMO_EMAIL: &str = "admin@acaciagrove.example";
const DEMO_PASSWORD: &str = "password123";

struct MockDb {
    organization: Organization,
    demo_user: User,
    projects: Vec<Project>,
    plots: Vec<Plot>,
    customers: Vec<Customer>,
    sales: Vec<PlotSale>,
    loan_accounts: Vec<PlotLoanAccount>,
    payments: Vec<Payment>,
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
            created_at: Utc::now(),
        };
        db.customers.push(customer.clone());
        Ok(customer)
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

        if !db.customers.iter().any(|c| c.id == input.customer_id) {
            return Err(ApiError::NotFound);
        }

        let already_sold = db.sales.iter().any(|s| s.plot_id == input.plot_id);
        if already_sold {
            return Err(ApiError::InvalidCredentials(
                "This plot already has an active sale.".to_string(),
            ));
        }

        let organization_id = db.organization.id;
        let agent_id = db.demo_user.id;
        let sale = PlotSale {
            id: Uuid::new_v4(),
            plot_id: input.plot_id,
            customer_id: input.customer_id,
            organization_id,
            agent_id: Some(agent_id),
            payment_mode: input.payment_mode,
            agreed_price: input.agreed_price,
            created_at: Utc::now(),
        };
        db.sales.push(sale.clone());

        if input.payment_mode != PaymentMode::FullCash {
            let seq = db.loan_accounts.len() + 1;
            let today = Utc::now().date_naive();
            db.loan_accounts
                .push(new_loan_account(&sale, seq, today));
        }

        if let Some(plot) = db.plots.iter_mut().find(|p| p.id == input.plot_id) {
            plot.status = match input.payment_mode {
                PaymentMode::FullCash => PlotStatus::Reserved,
                PaymentMode::LipaPolePoleInterestFree
                | PaymentMode::LipaPolePoleInterestBearing => PlotStatus::Booked,
            };
            plot.assigned_customer_id = Some(input.customer_id);
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
    let organization = Organization {
        id: Uuid::new_v4(),
        name: "Acacia Grove Properties".to_string(),
        code: "ACACIA".to_string(),
        currency: "KES".to_string(),
        created_at: Utc::now(),
    };

    let demo_user = User {
        id: Uuid::new_v4(),
        organization_id: organization.id,
        branch_id: None,
        full_name: "Amina Wanjiru".to_string(),
        email: DEMO_EMAIL.to_string(),
        is_active: true,
        is_platform_owner: false,
        created_at: Utc::now(),
    };

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
            plots.push(Plot {
                id: Uuid::new_v4(),
                project_id,
                plot_number: format!("{code}-{n:03}"),
                title_number: matches!(status, PlotStatus::Sold | PlotStatus::Transferred)
                    .then(|| format!("{code}/TITLE/{n:04}")),
                size: Decimal::new(125, 2),
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
        let customer = &customers[next_customer % customers.len()];
        next_customer += 1;

        plot.assigned_customer_id = Some(customer.id);
        let sale = PlotSale {
            id: Uuid::new_v4(),
            plot_id: plot.id,
            customer_id: customer.id,
            organization_id: organization.id,
            agent_id: Some(demo_user.id),
            payment_mode,
            agreed_price: plot.asking_price,
            created_at: Utc::now(),
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

    MockDb {
        organization,
        demo_user,
        projects,
        plots,
        customers,
        sales,
        loan_accounts,
        payments,
    }
}
