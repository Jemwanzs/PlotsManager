//! A single payment's printable receipt — distinct from the loan
//! statement (a running summary across every transaction on the
//! account, `pages/loan_statement.rs`). "Download PDF" reuses that
//! same page's established pattern: the browser's native print
//! (`window.print()`), scoped to a clean printable layout via the
//! shared `.no-print`/`@media print` rules, not a server-generated
//! file — genuinely produces a usable PDF via "Save as PDF" without a
//! PDF-rendering dependency.

use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;
use uuid::Uuid;

use crate::auth::{use_api, use_currency};
use crate::components::{ErrorAlert, LoadingState};
use crate::format::format_money;

#[component]
pub fn ReceiptPage() -> impl IntoView {
    let api = use_api();
    let currency = use_currency();
    let params = use_params_map();
    let loan_account_id = move || -> Option<Uuid> { params.read().get("id").and_then(|id| Uuid::parse_str(&id).ok()) };
    let payment_id = move || -> Option<Uuid> { params.read().get("payment_id").and_then(|id| Uuid::parse_str(&id).ok()) };

    let detail = LocalResource::new(move || {
        let api = api.clone();
        async move {
            match loan_account_id() {
                Some(id) => Some(api.get_loan_account(id).await),
                None => None,
            }
        }
    });

    view! {
        <Suspense fallback=|| view! { <LoadingState label="Loading receipt…" /> }>
            {move || {
                let currency = currency.get();
                detail
                    .get()
                    .map(|wrapped| wrapped.take())
                    .flatten()
                    .map(move |result| match result {
                        Ok(d) => {
                            let Some(payment) = d.payments.iter().find(|p| Some(p.id) == payment_id()).cloned() else {
                                return view! { <ErrorAlert message="That payment couldn't be found on this account.".to_string() /> }.into_any();
                            };
                            let back_href = format!("/loan-accounts/{}", d.account.id);
                            view! {
                                <div class="page-header no-print">
                                    <div>
                                        <h1>"Receipt — " {payment.receipt_number.clone()}</h1>
                                        <p>{d.customer_name.clone()}</p>
                                    </div>
                                    <div style="display: flex; gap: var(--space-2)">
                                        <A href=back_href attr:class="btn btn-secondary">"Back to account"</A>
                                        <button
                                            type="button"
                                            class="btn btn-primary"
                                            on:click=move |_| { let _ = web_sys::window().and_then(|w| w.print().ok()); }
                                        >
                                            "Download PDF"
                                        </button>
                                    </div>
                                </div>

                                <div class="card form-card" style="max-width: 640px;">
                                    <div class="page-header" style="margin-bottom: var(--space-4)">
                                        <div>
                                            <h2 class="mt-0">"Payment Receipt"</h2>
                                            <p class="meta mt-0">{payment.receipt_number.clone()}</p>
                                        </div>
                                        <p class="meta mt-0">{payment.payment_date.to_string()}</p>
                                    </div>

                                    <div class="section-grid-2" style="margin-bottom: var(--space-4)">
                                        <div><span class="meta">"Received from"</span><p class="mt-0">{d.customer_name.clone()}</p></div>
                                        <div><span class="meta">"Loan account"</span><p class="mt-0">{d.account.account_number.clone()}</p></div>
                                        <div><span class="meta">"Plot"</span><p class="mt-0">{d.plot_number.clone()}</p></div>
                                        <div><span class="meta">"Project"</span><p class="mt-0">{d.project_name.clone()}</p></div>
                                    </div>

                                    <div style="border-top: 1px solid var(--color-border); border-bottom: 1px solid var(--color-border); padding: var(--space-4) 0; margin-bottom: var(--space-4);">
                                        <div class="section-grid-2">
                                            <div><span class="meta">"Amount received"</span><p class="mt-0" style="font-size: 1.3rem; font-weight: 700;">{format_money(payment.amount, &currency)}</p></div>
                                            <div><span class="meta">"Payment method"</span><p class="mt-0">{payment.method.clone()}</p></div>
                                            {payment.external_reference.clone().map(|r| view! {
                                                <div><span class="meta">"Reference"</span><p class="mt-0">{r}</p></div>
                                            })}
                                        </div>
                                    </div>

                                    <div class="section-grid-2">
                                        <div><span class="meta">"Total paid to date"</span><p class="mt-0">{format_money(d.account.amount_paid, &currency)}</p></div>
                                        <div><span class="meta">"Outstanding balance (as of today)"</span><p class="mt-0">{format_money(d.account.outstanding_balance, &currency)}</p></div>
                                    </div>

                                    <p class="meta" style="margin-top: var(--space-5)">
                                        "This receipt confirms the above payment was received against the referenced loan account. "
                                        "It does not itemise the interest/penalty/principal breakdown — see the "
                                        "full statement for that."
                                    </p>
                                </div>
                            }.into_any()
                        }
                        Err(e) => view! { <ErrorAlert message=format!("Couldn't load this receipt: {e}") /> }.into_any(),
                    })
            }}
        </Suspense>
    }
}
