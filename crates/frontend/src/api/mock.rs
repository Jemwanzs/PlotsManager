//! In-memory sample data standing in for the real Rust API while the UI
//! is built ahead of it (see docs/14-development-roadmap.md). Every
//! method here has the exact signature `api::http::HttpApi` will
//! eventually have, so swapping `ApiClient::new_mock()` for
//! `ApiClient::new_http(base_url)` at the one call site in `app.rs` is
//! the entire migration — no component touches this module directly.

use std::sync::{Arc, Mutex};

use chrono::Utc;
use domain::{AreaUnit, Customer, Organization, Plot, PlotStatus, Project, ProjectStatus, User};
use rust_decimal::Decimal;
use uuid::Uuid;

use super::plot_status::status_meta;
use super::types::{ApiError, AuthSession, DashboardSummary, PlotWithColor, ProjectSummary};

const DEMO_EMAIL: &str = "admin@acaciagrove.example";
const DEMO_PASSWORD: &str = "password123";

struct MockDb {
    #[allow(dead_code)] // read via list_projects/list_plots joins, not directly yet
    organization: Organization,
    demo_user: User,
    projects: Vec<Project>,
    plots: Vec<Plot>,
    #[allow(dead_code)] // seeded for realism; no customer screens built yet
    customers: Vec<Customer>,
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

    MockDb {
        organization,
        demo_user,
        projects,
        plots,
        customers,
    }
}
