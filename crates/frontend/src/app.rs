use leptos::prelude::*;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::{ParamSegment, StaticSegment};

use crate::api::ApiClient;
use crate::auth::AuthSignal;
use crate::layout::AppShell;
use crate::pages::{
    CustomerDetail, CustomersList, Dashboard, LoanAccountDetailPage, Login, NewCustomer,
    NewProject, NotFound, PlatformOrganizationDetailPage, PlatformOrganizations, ProjectDetail,
    ProjectsList, Signup,
};

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
    provide_context(build_api_client());
    provide_context::<AuthSignal>(RwSignal::new(None));

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
                    path=StaticSegment("platform")
                    view=|| view! { <AppShell><PlatformOrganizations /></AppShell> }
                />
                <Route
                    path=(StaticSegment("platform"), ParamSegment("id"))
                    view=|| view! { <AppShell><PlatformOrganizationDetailPage /></AppShell> }
                />
            </Routes>
        </Router>
    }
}
