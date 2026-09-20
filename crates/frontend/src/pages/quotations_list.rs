use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_query_map;

use crate::auth::{use_api, use_currency};
use crate::components::{EmptyState, ErrorAlert, LoadingState, StatusBadge};
use crate::format::{format_money, format_payment_mode};
use domain::QuotationStatus;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Filter {
    All,
    Active,
}

impl Filter {
    fn matches(self, status: QuotationStatus, is_expired: bool) -> bool {
        match self {
            Filter::All => true,
            Filter::Active => {
                matches!(status, QuotationStatus::Draft | QuotationStatus::Sent) && !is_expired
            }
        }
    }
}

#[component]
pub fn QuotationsList() -> impl IntoView {
    let api = use_api();
    let currency = use_currency();

    // `/quotations?filter=active` (the Settings/Approvals/Reports nav
    // sub-items follow the same pattern) selects the tab; the tabs stay
    // switchable afterward like any other in-page filter. Re-derived on
    // every query change, not just read once at mount — see reports.rs's
    // identical Effect for why (leptos_router reuses this component
    // across query-only navigation, it doesn't remount it).
    let query = use_query_map();
    let filter = RwSignal::new(
        if query.get_untracked().get("filter").as_deref() == Some("active") {
            Filter::Active
        } else {
            Filter::All
        },
    );
    Effect::new(move |_| {
        filter.set(if query.get().get("filter").as_deref() == Some("active") {
            Filter::Active
        } else {
            Filter::All
        });
    });

    let quotations = LocalResource::new(move || {
        let api = api.clone();
        async move { api.list_quotations(None).await }
    });

    view! {
        <div class="page-header">
            <div>
                <h1>"Quotations"</h1>
                <p>"Formal price offers sent to customers, ahead of a committed sale."</p>
            </div>
        </div>

        <div class="filter-tabs">
            <button
                type="button"
                class="filter-tab"
                class:active=move || filter.get() == Filter::All
                on:click=move |_| filter.set(Filter::All)
            >
                "All"
            </button>
            <button
                type="button"
                class="filter-tab"
                class:active=move || filter.get() == Filter::Active
                on:click=move |_| filter.set(Filter::Active)
            >
                "Active"
            </button>
        </div>

        <Suspense fallback=|| view! { <LoadingState label="Loading quotations…" /> }>
            {move || {
                quotations
                    .get()
                    .map(|wrapped| wrapped.take())
                    .map(|result| match result {
                        Ok(list) => {
                            let list: Vec<_> = list
                                .into_iter()
                                .filter(|q| filter.get().matches(q.quotation.status, q.is_expired))
                                .collect();
                            if list.is_empty() {
                                return view! {
                                    <EmptyState
                                        icon="\u{1F4C4}"
                                        title="No quotations yet"
                                        detail="Send one from a project's plot detail panel — pick 'Send a quotation' instead of reserving directly."
                                    />
                                }
                                    .into_any();
                            }
                            view! {
                                <div class="card-grid">
                                    {list
                                        .into_iter()
                                        .map(|q| {
                                            let href = format!("/quotations/{}", q.quotation.id);
                                            view! {
                                                <A href=href attr:class="project-card card">
                                                    <div class="page-header" style="margin-bottom: var(--space-2)">
                                                        <h3 class="mt-0">{q.plot_number.clone()}</h3>
                                                        <StatusBadge label=q.status_label.clone() color=q.status_color.clone() />
                                                    </div>
                                                    <div class="meta">{q.project_name.clone()} " · " {q.customer_name.clone()}</div>
                                                    <p class="mt-0">
                                                        {format_payment_mode(q.quotation.payment_mode)} " · "
                                                        {format_money(q.quotation.quoted_price, &currency.get())}
                                                    </p>
                                                    <p class="meta mt-0">"Valid until " {q.quotation.valid_until.to_string()}</p>
                                                </A>
                                            }
                                        })
                                        .collect_view()}
                                </div>
                            }
                                .into_any()
                        }
                        Err(e) => view! { <ErrorAlert message=format!("Couldn't load quotations: {e}") /> }
                            .into_any(),
                    })
            }}
        </Suspense>
    }
}
