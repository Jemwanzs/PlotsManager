use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::{ParamSegment, StaticSegment};

use crate::api::ApiClient;
use crate::auth::{AuthSignal, CurrencySignal};
use crate::layout::AppShell;
use crate::pages::{
    ApprovalsList, Branches, BulkSalesImport, ChangePassword, CustomerDetail, CustomersList,
    Dashboard, FinanceLoanAccounts, FinanceOverview, LoanAccountDetailPage, LoanStatementPage, Login,
    MigrationBatchDetailPage, Migrations, NewCustomer,
    NewProject, NotFound, PlatformOrganizationDetailPage, PlatformOrganizations, ProjectDetail,
    ProjectsList, QuotationDetailPage, QuotationsList, ReceiptPage, RecentActivity, Reports, Roles, Settings,
    Signup, UsersAccess,
};
use crate::theme::{apply_theme, initial_theme, ThemeSignal};

/// `API_BASE_URL` is read at compile time (Trunk shells out to `cargo
/// build`, so whatever's in the environment when you run `trunk serve`/
/// `trunk build` — or Railway's build step — is what lands here). Unset
/// it for frontend-only work against `api::mock`; set it once
/// `crates/backend` is running to exercise the real API — see
/// .env.example.
fn build_api_client() -> ApiClient {
    match option_env!("API_BASE_URL") {
        Some(base_url) => ApiClient::new_http(base_url),
        None => ApiClient::new_mock(),
    }
}

#[component]
pub fn App() -> impl IntoView {
    let api = build_api_client();
    provide_context(api.clone());
    let auth: AuthSignal = RwSignal::new(None);
    provide_context(auth);
    let currency: CurrencySignal = RwSignal::new("KES".to_string());
    provide_context(currency);
    let theme: ThemeSignal = RwSignal::new(initial_theme());
    provide_context(theme);

    // Keeps `<html data-theme>` and `localStorage` in sync with the
    // signal — runs once on mount (so the resolved initial theme, saved
    // preference or OS default, is applied explicitly rather than left
    // to the CSS-only fallback) and again on every toggle click.
    Effect::new(move |_| {
        apply_theme(theme.get());
    });

    // Populated once per sign-in, not threaded through `AuthSession`
    // itself — `GET /api/v1/settings` already exists for the Settings
    // page, so reusing it here avoids growing the login/signup wire
    // contract just to carry one string. Resets to the "KES" default on
    // sign-out so a second, different-currency tenant signing in on the
    // same tab doesn't briefly show the previous tenant's currency.
    Effect::new(move |_| {
        match auth.get() {
            Some(_) => {
                let api = api.clone();
                spawn_local(async move {
                    if let Ok(settings) = api.get_settings().await {
                        currency.set(settings.currency);
                    }
                });
            }
            None => currency.set("KES".to_string()),
        }
    });

    view! {
        <Router>
            <Routes fallback=NotFound>
                <Route path=StaticSegment("login") view=Login />
                <Route path=StaticSegment("signup") view=Signup />
                <Route
                    path=StaticSegment("")
                    view=|| view! { <AppShell><Dashboard /></AppShell> }
                />
                <Route
                    path=StaticSegment("projects")
                    view=|| view! { <AppShell><ProjectsList /></AppShell> }
                />
                <Route
                    path=(StaticSegment("projects"), StaticSegment("new"))
                    view=|| view! { <AppShell><NewProject /></AppShell> }
                />
                <Route
                    path=(StaticSegment("projects"), ParamSegment("id"))
                    view=|| view! { <AppShell><ProjectDetail /></AppShell> }
                />
                <Route
                    path=StaticSegment("customers")
                    view=|| view! { <AppShell><CustomersList /></AppShell> }
                />
                <Route
                    path=(StaticSegment("customers"), StaticSegment("new"))
                    view=|| view! { <AppShell><NewCustomer /></AppShell> }
                />
                <Route
                    path=(StaticSegment("customers"), ParamSegment("id"))
                    view=|| view! { <AppShell><CustomerDetail /></AppShell> }
                />
                <Route
                    path=(StaticSegment("loan-accounts"), ParamSegment("id"))
                    view=|| view! { <AppShell><LoanAccountDetailPage /></AppShell> }
                />
                <Route
                    path=(StaticSegment("loan-accounts"), ParamSegment("id"), StaticSegment("statement"))
                    view=|| view! { <AppShell><LoanStatementPage /></AppShell> }
                />
                <Route
                    path=(StaticSegment("loan-accounts"), ParamSegment("id"), StaticSegment("payments"), ParamSegment("payment_id"), StaticSegment("receipt"))
                    view=|| view! { <AppShell><ReceiptPage /></AppShell> }
                />
                <Route
                    path=StaticSegment("platform")
                    view=|| view! { <AppShell><PlatformOrganizations /></AppShell> }
                />
                <Route
                    path=(StaticSegment("platform"), ParamSegment("id"))
                    view=|| view! { <AppShell><PlatformOrganizationDetailPage /></AppShell> }
                />
                <Route
                    path=StaticSegment("quotations")
                    view=|| view! { <AppShell><QuotationsList /></AppShell> }
                />
                <Route
                    path=(StaticSegment("quotations"), ParamSegment("id"))
                    view=|| view! { <AppShell><QuotationDetailPage /></AppShell> }
                />
                <Route
                    path=StaticSegment("approvals")
                    view=|| view! { <AppShell><ApprovalsList /></AppShell> }
                />
                <Route
                    path=StaticSegment("reports")
                    view=|| view! { <AppShell><Reports /></AppShell> }
                />
                <Route
                    path=(StaticSegment("sales"), StaticSegment("import"))
                    view=|| view! { <AppShell><BulkSalesImport /></AppShell> }
                />
                <Route
                    path=StaticSegment("settings")
                    view=|| view! { <AppShell><Settings /></AppShell> }
                />
                <Route
                    path=(StaticSegment("settings"), StaticSegment("roles"))
                    view=|| view! { <AppShell><Roles /></AppShell> }
                />
                <Route
                    path=(StaticSegment("settings"), StaticSegment("users"))
                    view=|| view! { <AppShell><UsersAccess /></AppShell> }
                />
                <Route
                    path=(StaticSegment("settings"), StaticSegment("branches"))
                    view=|| view! { <AppShell><Branches /></AppShell> }
                />
                <Route
                    path=(StaticSegment("account"), StaticSegment("change-password"))
                    view=|| view! { <AppShell><ChangePassword /></AppShell> }
                />
                <Route
                    path=StaticSegment("activity")
                    view=|| view! { <AppShell><RecentActivity /></AppShell> }
                />
                <Route
                    path=StaticSegment("finance")
                    view=|| view! { <AppShell><FinanceOverview /></AppShell> }
                />
                <Route
                    path=(StaticSegment("finance"), StaticSegment("loan-accounts"))
                    view=|| view! { <AppShell><FinanceLoanAccounts /></AppShell> }
                />
                <Route
                    path=StaticSegment("migrations")
                    view=|| view! { <AppShell><Migrations /></AppShell> }
                />
                <Route
                    path=(StaticSegment("migrations"), ParamSegment("id"))
                    view=|| view! { <AppShell><MigrationBatchDetailPage /></AppShell> }
                />
            </Routes>
        </Router>
    }
}
