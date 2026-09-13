use leptos::prelude::*;
use leptos_router::components::A;

use crate::api::CustomerSummary;
use crate::auth::use_api;
use crate::components::{EmptyState, ErrorAlert, LoadingState};

#[component]
pub fn CustomersList() -> impl IntoView {
    let api = use_api();
    let search = RwSignal::new(String::new());

    let customers = LocalResource::new(move || {
        let api = api.clone();
        async move { api.list_customers().await }
    });

    view! {
        <div class="page-header">
            <div>
                <h1>"Customers"</h1>
                <p>"Everyone who has expressed interest in, reserved, or bought a plot."</p>
            </div>
            <A href="/customers/new" attr:class="btn btn-primary">"+ New customer"</A>
        </div>

        <input
            class="search-input"
            type="search"
            placeholder="Search by name, phone, or email…"
            prop:value=search
            on:input=move |ev| search.set(event_target_value(&ev))
        />

        <Suspense fallback=|| view! { <LoadingState label="Loading customers…" /> }>
            {move || {
                customers
                    .get()
                    .map(|wrapped| wrapped.take())
                    .map(|result| match result {
                        Ok(list) => {
                            let query = search.get().to_lowercase();
                            let filtered: Vec<CustomerSummary> = list
                                .into_iter()
                                .filter(|c| {
                                    query.is_empty()
                                        || c.customer.full_name.to_lowercase().contains(&query)
                                        || c.customer.phone.as_deref().unwrap_or("").contains(&query)
                                        || c.customer.email.as_deref().unwrap_or("").to_lowercase().contains(&query)
                                })
                                .collect();

                            if filtered.is_empty() {
                                view! {
                                    <EmptyState
                                        icon="\u{1F464}"
                                        title="No customers match your search"
                                        detail="Try a different name, phone number, or email."
                                    />
                                }
                                    .into_any()
                            } else {
                                view! {
                                    <div class="card-grid">
                                        {filtered
                                            .into_iter()
                                            .map(|c| view! { <CustomerCard summary=c /> })
                                            .collect_view()}
                                    </div>
                                }
                                    .into_any()
                            }
                        }
                        Err(e) => {
                            view! { <ErrorAlert message=format!("Couldn't load customers: {e}") /> }.into_any()
                        }
                    })
            }}
        </Suspense>
    }
}

#[component]
fn CustomerCard(summary: CustomerSummary) -> impl IntoView {
    let href = format!("/customers/{}", summary.customer.id);
    view! {
        <A href=href attr:class="project-card card">
            <h3>{summary.customer.full_name.clone()}</h3>
            <div class="meta">
                {summary.customer.phone.clone().unwrap_or_else(|| "No phone on file".to_string())}
                " · "
                {summary.customer.email.clone().unwrap_or_else(|| "No email on file".to_string())}
            </div>
            <p class="mt-0">
                {if summary.plots_owned == 0 {
                    "No plots yet".to_string()
                } else if summary.plots_owned == 1 {
                    "1 plot".to_string()
                } else {
                    format!("{} plots", summary.plots_owned)
                }}
            </p>
        </A>
    }
}
