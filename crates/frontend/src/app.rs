use leptos::prelude::*;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::{ParamSegment, StaticSegment};

use crate::api::ApiClient;
use crate::auth::AuthSignal;
use crate::layout::AppShell;
use crate::pages::{
    CustomerDetail, CustomersList, Dashboard, Login, NotFound, ProjectDetail, ProjectsList,
};

#[component]
pub fn App() -> impl IntoView {
    provide_context(ApiClient::new_mock());
    provide_context::<AuthSignal>(RwSignal::new(None));

    view! {
        <Router>
            <Routes fallback=NotFound>
                <Route path=StaticSegment("login") view=Login />
                <Route
                    path=StaticSegment("")
                    view=|| view! { <AppShell><Dashboard /></AppShell> }
                />
                <Route
                    path=StaticSegment("projects")
                    view=|| view! { <AppShell><ProjectsList /></AppShell> }
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
                    path=(StaticSegment("customers"), ParamSegment("id"))
                    view=|| view! { <AppShell><CustomerDetail /></AppShell> }
                />
            </Routes>
        </Router>
    }
}
