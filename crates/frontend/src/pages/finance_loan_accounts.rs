//! Finance → Loan Accounts — every Lipa Pole Pole receivable across
//! every project in the organization, filterable by performance. Nothing
//! surfaced this as a single list before the Finance module: a loan
//! account was only reachable by drilling into the customer that holds
//! it (`pages/customer_detail.rs`). Each row still links to the existing
//! `pages/loan_account_detail.rs` — this page doesn't duplicate that
//! detail/payment-recording UI, just makes the accounts findable.

use leptos::prelude::*;
use leptos_router::components::A;

use crate::auth::{use_api, use_currency};
use crate::components::{EmptyState, ErrorAlert, LoadingState, StatusBadge};
use crate::format::format_money;
use domain::{LoanAccountStatus, LoanAccountSummary};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Filter {
    All,
    Performing,
    NonPerforming,
    FullyPaid,
}

impl Filter {
    fn matches(self, status: LoanAccountStatus) -> bool {
        use LoanAccountStatus::*;
        match self {
            Filter::All => true,
            Filter::Performing => {
                matches!(status, ActiveCurrent | ActivePartiallyPaid | InGracePeriod)
            }
            Filter::NonPerforming => {
                matches!(status, InArrears | Defaulted | RepossessedOrReallocated)
            }
            Filter::FullyPaid => matches!(status, FullyPaid),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Filter::All => "All",
            Filter::Performing => "Performing",
            Filter::NonPerforming => "Non-performing",
            Filter::FullyPaid => "Fully paid",
        }
    }
}

#[component]
pub fn FinanceLoanAccounts() -> impl IntoView {
    let api = use_api();
    let currency = use_currency();
    let filter = RwSignal::new(Filter::All);

    let accounts = LocalResource::new({
        let api = api.clone();
        move || {
            let api = api.clone();
            async move { api.list_loan_accounts().await }
        }
    });

    view! {
        <div class="page-header">
            <div>
                <h1>"Loan accounts"</h1>
                <p>"Every Lipa Pole Pole receivable across every project, in one place."</p>
            </div>
        </div>

        <div class="filter-tabs">
            {[Filter::All, Filter::Performing, Filter::NonPerforming, Filter::FullyPaid]
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

        <Suspense fallback=|| view! { <LoadingState label="Loading loan accounts…" /> }>
            {move || {
                accounts
                    .get()
                    .map(|wrapped| wrapped.take())
                    .map(|result| match result {
                        Ok(list) => {
                            let filtered: Vec<LoanAccountSummary> = list
                                .into_iter()
                                .filter(|a| filter.get().matches(a.account.status))
                                .collect();
                            if filtered.is_empty() {
                                view! {
                                    <EmptyState
                                        icon="\u{1F4B0}"
                                        title="No loan accounts"
                                        detail="Accounts matching this filter will appear here."
                                    />
                                }
                                    .into_any()
                            } else {
                                view! {
                                    <div class="card-grid">
                                        {filtered
                                            .into_iter()
                                            .map(|a| {
                                                let href = format!("/loan-accounts/{}", a.account.id);
                                                view! {
                                                    <A href=href attr:class="project-card card">
                                                        <div class="page-header" style="margin-bottom: var(--space-2)">
                                                            <h3 class="mt-0">{a.account.account_number.clone()}</h3>
                                                            <StatusBadge label=a.status_label.clone() color=a.status_color.clone() />
                                                        </div>
                                                        <div class="meta">
                                                            {a.customer_name.clone()} " · " {a.project_name.clone()} " · Plot " {a.plot_number.clone()}
                                                        </div>
                                                        <p class="mt-0">
                                                            "Outstanding: " {format_money(a.account.outstanding_balance, &currency.get())}
                                                        </p>
                                                    </A>
                                                }
                                            })
                                            .collect_view()}
                                    </div>
                                }
                                    .into_any()
                            }
                        }
                        Err(e) => view! { <ErrorAlert message=format!("Couldn't load loan accounts: {e}") /> }
                            .into_any(),
                    })
            }}
        </Suspense>
    }
}
