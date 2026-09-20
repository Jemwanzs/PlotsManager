//! Finance → Receivable / Lipa Pole Pole Account → View Statement — a
//! running statement generated from the actual transaction ledger
//! (`GET /api/v1/loan-accounts/:id/statement`,
//! `crates/backend/src/routes/loan_accounts.rs`), never reconstructed
//! from the account's current balance. `Date Range | View` filters the
//! already-fetched entries client-side (a loan account's history is
//! small); `Export CSV` is a real, working export — "Download PDF" is
//! a follow-up, not faked here.

use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;
use uuid::Uuid;

use crate::auth::use_api;
use crate::components::{ErrorAlert, LoadingState};
use crate::format::{format_ledger_entry_type, format_money};
use domain::LoanLedgerEntry;

#[component]
pub fn LoanStatementPage() -> impl IntoView {
    let api = use_api();
    let params = use_params_map();
    let loan_account_id = move || -> Option<Uuid> { params.read().get("id").and_then(|id| Uuid::parse_str(&id).ok()) };

    let statement = LocalResource::new(move || {
        let api = api.clone();
        async move {
            match loan_account_id() {
                Some(id) => Some(api.get_loan_statement(id).await),
                None => None,
            }
        }
    });

    view! {
        <Suspense fallback=|| view! { <LoadingState label="Loading statement…" /> }>
            {move || {
                statement
                    .get()
                    .map(|wrapped| wrapped.take())
                    .flatten()
                    .map(|result| match result {
                        Ok(s) => view! { <StatementContent statement=s /> }.into_any(),
                        Err(e) => view! { <ErrorAlert message=format!("Couldn't load this statement: {e}") /> }.into_any(),
                    })
            }}
        </Suspense>
    }
}

#[component]
fn StatementContent(statement: domain::LoanStatement) -> impl IntoView {
    let currency = crate::auth::use_currency();
    let from_filter = RwSignal::new(String::new());
    let to_filter = RwSignal::new(String::new());

    let account_href = format!("/loan-accounts/{}", statement.account.id);
    let entries_for_filter = statement.entries.clone();
    let entries_for_export = statement.entries.clone();

    let filtered_entries = move || -> Vec<LoanLedgerEntry> {
        let from = chrono::NaiveDate::parse_from_str(from_filter.get().trim(), "%Y-%m-%d").ok();
        let to = chrono::NaiveDate::parse_from_str(to_filter.get().trim(), "%Y-%m-%d").ok();
        entries_for_filter
            .iter()
            .filter(|e| from.is_none_or(|f| e.entry_date >= f) && to.is_none_or(|t| e.entry_date <= t))
            .cloned()
            .collect()
    };

    let account_number_for_export = statement.account.account_number.clone();
    let csv_href = move || {
        let entries = filtered_entries_for_export(&entries_for_export, &from_filter.get(), &to_filter.get());
        let mut csv = String::from("Date,Transaction,Amount,Principal Cleared,Interest Paid,Penalty Paid,Balance\n");
        for e in &entries {
            csv.push_str(&format!(
                "{},{},{},{},{},{},{}\n",
                e.entry_date,
                format_ledger_entry_type(e.entry_type),
                e.gross_amount,
                component_value(e.principal_delta),
                component_value(e.interest_delta),
                component_value(e.penalty_delta),
                e.balance_after,
            ));
        }
        format!("data:text/csv;charset=utf-8,{}", js_sys::encode_uri_component(&csv))
    };
    let csv_filename = format!("statement-{account_number_for_export}.csv");

    view! {
        <div class="page-header">
            <div>
                <h1>"Statement — " {statement.account.account_number.clone()}</h1>
                <p>{statement.plot_number.clone()} " · " {statement.project_name.clone()}</p>
            </div>
            <A href=account_href attr:class="btn btn-secondary">"Back to account"</A>
        </div>

        <div class="card form-card" style="max-width: none; margin-bottom: var(--space-4);">
            <div class="section-grid-2">
                <div><span class="meta">"Customer"</span><p class="mt-0">{statement.customer_name.clone()}</p></div>
                <div><span class="meta">"Plot"</span><p class="mt-0">{statement.plot_number.clone()}</p></div>
                <div><span class="meta">"Project"</span><p class="mt-0">{statement.project_name.clone()}</p></div>
                <div><span class="meta">"Sale Price / Principal"</span><p class="mt-0">{format_money(statement.account.principal, &currency.get())}</p></div>
                <div>
                    <span class="meta">"Interest Rate"</span>
                    <p class="mt-0">{statement.account.interest_rate.map(|r| format!("{r}% p.a.")).unwrap_or_else(|| "No interest".to_string())}</p>
                </div>
                <div><span class="meta">"Instalment"</span><p class="mt-0">{format_money(statement.account.instalment_amount, &currency.get())}</p></div>
                <div><span class="meta">"Total Paid"</span><p class="mt-0">{format_money(statement.account.amount_paid, &currency.get())}</p></div>
                <div><span class="meta">"Outstanding"</span><p class="mt-0">{format_money(statement.account.outstanding_balance, &currency.get())}</p></div>
                <div><span class="meta">"Status"</span><p class="mt-0"><span class="badge" style=format!("background-color: {}", statement.status_color)>{statement.status_label.clone()}</span></p></div>
            </div>
        </div>

        <div class="card" style="margin-bottom: var(--space-4);">
            <div style="display:flex; gap: var(--space-3); flex-wrap: wrap; align-items: flex-end;">
                <div class="field" style="margin-bottom: 0;">
                    <label for="stmt-from">"From"</label>
                    <input id="stmt-from" type="date" prop:value=from_filter on:change=move |ev| from_filter.set(event_target_value(&ev)) />
                </div>
                <div class="field" style="margin-bottom: 0;">
                    <label for="stmt-to">"To"</label>
                    <input id="stmt-to" type="date" prop:value=to_filter on:change=move |ev| to_filter.set(event_target_value(&ev)) />
                </div>
                <a href=csv_href download=csv_filename class="btn btn-secondary" style="display:inline-block;">"Export CSV"</a>
            </div>
        </div>

        <div class="card">
            <div class="table-scroll">
                <table class="data-table">
                    <thead>
                        <tr>
                            <th>"Date"</th>
                            <th>"Transaction"</th>
                            <th>"Amount"</th>
                            <th>"Principal Cleared"</th>
                            <th>"Interest Paid"</th>
                            <th>"Penalty Paid"</th>
                            <th>"Balance"</th>
                        </tr>
                    </thead>
                    <tbody>
                        {move || {
                            let currency = currency.get();
                            filtered_entries().into_iter().map(|e| {
                                view! {
                                    <tr>
                                        <td>{e.entry_date.to_string()}</td>
                                        <td>{format_ledger_entry_type(e.entry_type)}</td>
                                        <td>{format_money(e.gross_amount, &currency)}</td>
                                        <td>{component_display(e.principal_delta, &currency)}</td>
                                        <td>{component_display(e.interest_delta, &currency)}</td>
                                        <td>{component_display(e.penalty_delta, &currency)}</td>
                                        <td>{format_money(e.balance_after, &currency)}</td>
                                    </tr>
                                }
                            }).collect_view()
                        }}
                    </tbody>
                </table>
            </div>
        </div>
    }
}

fn component_value(delta: rust_decimal::Decimal) -> rust_decimal::Decimal {
    delta.abs()
}

fn component_display(delta: rust_decimal::Decimal, currency: &str) -> String {
    if delta.is_zero() {
        "—".to_string()
    } else {
        format_money(delta.abs(), currency)
    }
}

fn filtered_entries_for_export(
    entries: &[LoanLedgerEntry],
    from: &str,
    to: &str,
) -> Vec<LoanLedgerEntry> {
    let from = chrono::NaiveDate::parse_from_str(from.trim(), "%Y-%m-%d").ok();
    let to = chrono::NaiveDate::parse_from_str(to.trim(), "%Y-%m-%d").ok();
    entries
        .iter()
        .filter(|e| from.is_none_or(|f| e.entry_date >= f) && to.is_none_or(|t| e.entry_date <= t))
        .cloned()
        .collect()
}
