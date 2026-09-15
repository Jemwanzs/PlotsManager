use leptos::prelude::*;
use leptos_router::components::A;

use crate::api::{lead_stage_meta, CustomerSummary, LeadStage};
use crate::auth::use_api;
use crate::components::{EmptyState, ErrorAlert, LoadingState, StatusBadge};

#[derive(Clone, Copy, PartialEq)]
enum PipelineFilter {
    All,
    Leads,
    Converted,
    Lost,
}

impl PipelineFilter {
    fn matches(self, summary: &CustomerSummary) -> bool {
        match self {
            PipelineFilter::All => true,
            PipelineFilter::Converted => summary.plots_owned > 0,
            PipelineFilter::Lost => {
                summary.plots_owned == 0 && summary.customer.stage == LeadStage::Lost
            }
            PipelineFilter::Leads => {
                summary.plots_owned == 0 && summary.customer.stage != LeadStage::Lost
            }
        }
    }

    fn label(self) -> &'static str {
        match self {
            PipelineFilter::All => "All",
            PipelineFilter::Leads => "Leads",
            PipelineFilter::Converted => "Converted",
            PipelineFilter::Lost => "Lost",
        }
    }
}

#[component]
pub fn CustomersList() -> impl IntoView {
    let api = use_api();
    let search = RwSignal::new(String::new());
    let filter = RwSignal::new(PipelineFilter::All);

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

        <div class="filter-tabs">
            {[
                PipelineFilter::All,
                PipelineFilter::Leads,
                PipelineFilter::Converted,
                PipelineFilter::Lost,
            ]
                .into_iter()
                .map(|f| {
                    view! {
                        <button
                            type="button"
                            class="filter-tab"
                            class:active=move || filter.get() == f
                            on:click=move |_| filter.set(f)
                        >
                            {f.label()}
                        </button>
                    }
                })
                .collect_view()}
        </div>

        <Suspense fallback=|| view! { <LoadingState label="Loading customers…" /> }>
            {move || {
                customers
                    .get()
                    .map(|wrapped| wrapped.take())
                    .map(|result| match result {
                        Ok(list) => {
                            let query = search.get().to_lowercase();
                            let active_filter = filter.get();
                            let filtered: Vec<CustomerSummary> = list
                                .into_iter()
                                .filter(|c| active_filter.matches(c))
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
                                        title="Nothing here"
                                        detail="Try a different filter, or a different name, phone number, or email."
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
    let converted = summary.plots_owned > 0;
    let (stage_label, stage_color) = if converted {
        ("Converted", "#15734f")
    } else {
        lead_stage_meta(summary.customer.stage)
    };

    view! {
        <A href=href attr:class="project-card card">
            <div class="page-header" style="margin-bottom: var(--space-2)">
                <h3 class="mt-0">{summary.customer.full_name.clone()}</h3>
                <StatusBadge label=stage_label.to_string() color=stage_color.to_string() />
            </div>
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
