use leptos::prelude::*;
use leptos_router::components::A;

use crate::auth::{use_api, use_currency};
use crate::components::{EmptyState, ErrorAlert, LoadingState, StatusBadge};
use crate::format::{format_money, format_payment_mode};

#[component]
pub fn QuotationsList() -> impl IntoView {
    let api = use_api();
    let currency = use_currency();

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

        <Suspense fallback=|| view! { <LoadingState label="Loading quotations…" /> }>
            {move || {
                quotations
                    .get()
                    .map(|wrapped| wrapped.take())
                    .map(|result| match result {
                        Ok(list) if list.is_empty() => view! {
                            <EmptyState
                                icon="\u{1F4C4}"
                                title="No quotations yet"
                                detail="Send one from a project's plot detail panel — pick 'Send a quotation' instead of reserving directly."
                            />
                        }
                            .into_any(),
                        Ok(list) => view! {
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
                            .into_any(),
                        Err(e) => view! { <ErrorAlert message=format!("Couldn't load quotations: {e}") /> }
                            .into_any(),
                    })
            }}
        </Suspense>
    }
}
