use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;
use uuid::Uuid;

use crate::auth::use_api;
use crate::components::{EmptyState, ErrorAlert, LoadingState, StatusBadge};
use crate::format::{format_kes, format_payment_mode};

#[component]
pub fn CustomerDetail() -> impl IntoView {
    let api = use_api();
    let params = use_params_map();
    let customer_id = move || -> Option<Uuid> { params.read().get("id").and_then(|id| Uuid::parse_str(&id).ok()) };

    let detail = LocalResource::new(move || {
        let api = api.clone();
        async move {
            match customer_id() {
                Some(id) => Some(api.get_customer(id).await),
                None => None,
            }
        }
    });

    view! {
        <Suspense fallback=|| view! { <LoadingState label="Loading customer…" /> }>
            {move || {
                detail
                    .get()
                    .map(|wrapped| wrapped.take())
                    .flatten()
                    .map(|result| match result {
                        Ok(d) => view! {
                            <div class="page-header">
                                <div>
                                    <h1>{d.customer.full_name.clone()}</h1>
                                    <p>
                                        {d.customer.phone.clone().unwrap_or_else(|| "No phone on file".to_string())}
                                        " · "
                                        {d.customer.email.clone().unwrap_or_else(|| "No email on file".to_string())}
                                        {d.customer.id_number.clone().map(|id| format!(" · ID {id}")).unwrap_or_default()}
                                    </p>
                                </div>
                            </div>

                            <h2>"Plots"</h2>
                            {if d.sales.is_empty() {
                                view! {
                                    <EmptyState
                                        icon="\u{1F3D8}\u{FE0F}"
                                        title="No plots yet"
                                        detail="Reserve a plot for this customer from a project's plot map."
                                    />
                                }
                                    .into_any()
                            } else {
                                view! {
                                    <div class="card-grid">
                                        {d.sales
                                            .into_iter()
                                            .map(|sale| {
                                                // A Lipa Pole Pole sale has a loan account — that's the
                                                // more useful destination (payment history/capture)
                                                // than the project. A Full Cash sale has neither yet
                                                // (docs/08 §2.1 payment tracking isn't built), so it
                                                // just links back to the project for now.
                                                let href = sale
                                                    .loan_account_id
                                                    .map(|id| format!("/loan-accounts/{id}"))
                                                    .unwrap_or_else(|| format!("/projects/{}", sale.project_id));
                                                view! {
                                                    <A href=href attr:class="project-card card">
                                                        <div class="page-header" style="margin-bottom: var(--space-2)">
                                                            <h3 class="mt-0">{sale.plot_number.clone()}</h3>
                                                            <StatusBadge label=sale.status_label.clone() color=sale.status_color.clone() />
                                                        </div>
                                                        <div class="meta">{sale.project_name.clone()}</div>
                                                        <p class="mt-0">
                                                            {format_payment_mode(sale.payment_mode)} " · "
                                                            {format_kes(sale.agreed_price)}
                                                        </p>
                                                    </A>
                                                }
                                            })
                                            .collect_view()}
                                    </div>
                                }
                                    .into_any()
                            }}
                        }
                            .into_any(),
                        Err(e) => view! { <ErrorAlert message=format!("Couldn't load this customer: {e}") /> }
                            .into_any(),
                    })
            }}
        </Suspense>
    }
}
