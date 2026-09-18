//! Home → Recent activity — the most recently recorded sales across the
//! whole organization, one glance without opening Reports. Reuses the
//! existing sales report endpoint (unfiltered) rather than adding a
//! dedicated activity-feed endpoint — an activity log covering more than
//! sales (approvals, edits, ...) is real audit-log territory
//! (docs/14's roadmap, tracked separately), not something to half-build
//! here just for a nav sub-item.

use leptos::prelude::*;
use leptos_router::components::A;

use crate::auth::{use_api, use_currency};
use crate::components::{EmptyState, ErrorAlert, LoadingState};
use crate::format::{format_money, format_payment_mode};

#[component]
pub fn RecentActivity() -> impl IntoView {
    let api = use_api();
    let currency = use_currency();

    let report = LocalResource::new({
        let api = api.clone();
        move || {
            let api = api.clone();
            async move { api.sales_report(None, None, None, None).await }
        }
    });

    view! {
        <div class="page-header">
            <div>
                <h1>"Recent activity"</h1>
                <p>"The most recent sales recorded across the organization."</p>
            </div>
        </div>

        <Suspense fallback=|| view! { <LoadingState label="Loading recent activity…" /> }>
            {move || {
                report
                    .get()
                    .map(|wrapped| wrapped.take())
                    .map(|result| match result {
                        Ok(r) => {
                            let mut rows = r.rows;
                            rows.sort_by(|a, b| b.created_at.cmp(&a.created_at));
                            rows.truncate(15);
                            if rows.is_empty() {
                                view! {
                                    <EmptyState
                                        icon="\u{1F553}"
                                        title="No activity yet"
                                        detail="Recorded sales will appear here as they happen."
                                    />
                                }
                                    .into_any()
                            } else {
                                view! {
                                    <div class="card" style="padding: 0; overflow-x: auto;">
                                        <table style="width: 100%; border-collapse: collapse;">
                                            <thead>
                                                <tr style="text-align: left; border-bottom: 1px solid var(--color-border);">
                                                    <th style="padding: var(--space-3)">"Date"</th>
                                                    <th style="padding: var(--space-3)">"Plot"</th>
                                                    <th style="padding: var(--space-3)">"Customer"</th>
                                                    <th style="padding: var(--space-3)">"Mode"</th>
                                                    <th style="padding: var(--space-3)">"Value"</th>
                                                </tr>
                                            </thead>
                                            <tbody>
                                                {rows.into_iter().map(|row| {
                                                    let href = format!("/customers/{}", row.customer_id);
                                                    view! {
                                                        <tr style="border-bottom: 1px solid var(--color-border);">
                                                            <td style="padding: var(--space-3)">{row.created_at.date_naive().to_string()}</td>
                                                            <td style="padding: var(--space-3)">{row.project_name.clone()} " · " {row.plot_number.clone()}</td>
                                                            <td style="padding: var(--space-3)"><A href=href>{row.customer_name.clone()}</A></td>
                                                            <td style="padding: var(--space-3)">{format_payment_mode(row.payment_mode)}</td>
                                                            <td style="padding: var(--space-3)">{format_money(row.agreed_price, &currency.get())}</td>
                                                        </tr>
                                                    }
                                                }).collect_view()}
                                            </tbody>
                                        </table>
                                    </div>
                                }
                                    .into_any()
                            }
                        }
                        Err(e) => view! { <ErrorAlert message=format!("Couldn't load recent activity: {e}") /> }
                            .into_any(),
                    })
            }}
        </Suspense>
    }
}
