mod approvals;
mod auth;
mod customers;
mod dashboard;
mod finance;
mod loan_accounts;
mod platform;
mod project_map;
mod projects;
mod quotations;
mod reports;
mod roles;
mod sales;
mod settings;

use axum::Router;

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .merge(auth::router())
        .merge(dashboard::router())
        .merge(finance::router())
        .merge(projects::router())
        .merge(customers::router())
        .merge(sales::router())
        .merge(loan_accounts::router())
        .merge(platform::router())
        .merge(quotations::router())
        .merge(approvals::router())
        .merge(reports::router())
        .merge(project_map::router())
        .merge(settings::router())
        .merge(roles::router())
}
