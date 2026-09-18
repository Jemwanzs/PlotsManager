//! Finance → Overview — the organization's receivables at a glance.
//! Reuses `dashboard_summary` (already computes active loan book,
//! performing/non-performing) and layers the org-wide loan-accounts list
//! (`GET /api/v1/finance/loan-accounts`, new with this module) on top for
//! figures the dashboard doesn't need: total outstanding balance and a
//! fully-paid count.

use leptos::prelude::*;
use leptos_router::components::A;
use rust_decimal::Decimal;

use crate::auth::{use_api, use_currency};
use crate::components::{ErrorAlert, LoadingState, StatCard, StatusBadge};
use crate::format::{format_amount, format_money};
use domain::{DashboardSummary, LoanAccountStatus, LoanAccountSummary};

#[component]
pub fn FinanceOverview() -> impl IntoView {
    let api = use_api();
    let currency = use_currency();

    let summary = LocalResource::new({
        let api = api.clone();
        move || {
            let api = api.clone();
            async move { api.dashboard_summary().await }
        }
    });
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
                <h1>"Finance overview"</h1>
                <p>"The organization's receivables at a glance — performing, non-performing, and fully paid."</p>
            </div>
            <div class="dashboard-currency">"Currency: " {move || currency.get()}</div>
        </div>

        <Suspense fallback=|| view! { <LoadingState label="Loading finance data…" /> }>
            {move || {
                let summary = summary.get().map(|w| w.take());
                let accounts = accounts.get().map(|w| w.take());
                match (summary, accounts) {
                    (Some(Ok(s)), Some(Ok(a))) => {
                        view! { <FinanceOverviewContent summary=s accounts=a /> }.into_any()
                    }
                    (Some(Err(e)), _) => {
                        view! { <ErrorAlert message=format!("Couldn't load finance data: {e}") /> }.into_any()
                    }
                    (_, Some(Err(e))) => {
                        view! { <ErrorAlert message=format!("Couldn't load finance data: {e}") /> }.into_any()
                    }
                    _ => view! { <LoadingState label="Loading finance data…" /> }.into_any(),
                }
            }}
        </Suspense>
    }
}

#[component]
fn FinanceOverviewContent(summary: DashboardSummary, accounts: Vec<LoanAccountSummary>) -> impl IntoView {
    let currency = use_currency();
    let fully_paid = accounts
        .iter()
        .filter(|a| a.account.status == LoanAccountStatus::FullyPaid)
        .count();
    let total_outstanding: Decimal = accounts.iter().map(|a| a.account.outstanding_balance).sum();

    let mut top_accounts = accounts.clone();
    top_accounts.sort_by(|a, b| b.account.outstanding_balance.cmp(&a.account.outstanding_balance));
    top_accounts.truncate(6);

    view! {
        <div class="stat-grid">
            <StatCard
                label="Active loan book"
                value=format_amount(summary.active_loan_book)
                sub=format!("{} accounts", summary.active_loans_count)
            />
            <StatCard
                label="Outstanding balance"
                value=format_amount(total_outstanding)
                sub=format!("{} receivables", accounts.len())
            />
            <StatCard
                label="Performing"
                value=summary.performing_count.to_string()
                sub=format_amount(summary.performing_amount)
            />
            <StatCard
                label="Non-performing"
                value=summary.non_performing_count.to_string()
                sub=format_amount(summary.non_performing_amount)
            />
            <StatCard label="Fully paid" value=fully_paid.to_string() />
        </div>

        <div class="card">
            <div class="page-header" style="margin-bottom: var(--space-3)">
                <h2 class="mt-0">"Largest outstanding balances"</h2>
                <A href="/finance/loan-accounts" attr:class="btn btn-secondary">"View all"</A>
            </div>
            {if top_accounts.is_empty() {
                view! { <p class="text-muted">"No active receivables yet."</p> }.into_any()
            } else {
                view! {
                    <div style="overflow-x: auto;">
                        <table style="width: 100%; border-collapse: collapse;">
                            <thead>
                                <tr style="text-align: left; border-bottom: 1px solid var(--color-border);">
                                    <th style="padding: var(--space-3)">"Account"</th>
                                    <th style="padding: var(--space-3)">"Customer"</th>
                                    <th style="padding: var(--space-3)">"Project / plot"</th>
                                    <th style="padding: var(--space-3)">"Outstanding"</th>
                                    <th style="padding: var(--space-3)">"Status"</th>
                                </tr>
                            </thead>
                            <tbody>
                                {top_accounts.into_iter().map(|a| {
                                    let href = format!("/loan-accounts/{}", a.account.id);
                                    view! {
                                        <tr style="border-bottom: 1px solid var(--color-border);">
                                            <td style="padding: var(--space-3)">
                                                <A href=href>{a.account.account_number.clone()}</A>
                                            </td>
                                            <td style="padding: var(--space-3)">{a.customer_name.clone()}</td>
                                            <td style="padding: var(--space-3)">{a.project_name.clone()} " · " {a.plot_number.clone()}</td>
                                            <td style="padding: var(--space-3)">{format_money(a.account.outstanding_balance, &currency.get())}</td>
                                            <td style="padding: var(--space-3)">
                                                <StatusBadge label=a.status_label.clone() color=a.status_color.clone() />
                                            </td>
                                        </tr>
                                    }
                                }).collect_view()}
                            </tbody>
                        </table>
                    </div>
                }.into_any()
            }}
        </div>
    }
}
