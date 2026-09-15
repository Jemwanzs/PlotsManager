mod auth;
mod customers;
mod dashboard;
mod loan_accounts;
mod platform;
mod projects;
mod quotations;
mod sales;

use axum::Router;

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .merge(auth::router())
        .merge(dashboard::router())
        .merge(projects::router())
        .merge(customers::router())
        .merge(sales::router())
        .merge(loan_accounts::router())
        .merge(platform::router())
        .merge(quotations::router())
}
