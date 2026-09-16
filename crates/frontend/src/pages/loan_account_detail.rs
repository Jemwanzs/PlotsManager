use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;
use rust_decimal::Decimal;
use std::str::FromStr;
use uuid::Uuid;

use crate::api::{LoanAccountDetail, RecordPaymentInput};
use crate::auth::{use_api, use_currency};
use crate::components::{ErrorAlert, LoadingState, StatCard, StatusBadge};
use crate::format::{format_money, format_payment_status};

#[component]
pub fn LoanAccountDetailPage() -> impl IntoView {
    let api = use_api();
    let params = use_params_map();
    let account_id = move || -> Option<Uuid> { params.read().get("id").and_then(|id| Uuid::parse_str(&id).ok()) };

    // Bumping this signal is how the "record payment" form tells this
    // page to refetch — LocalResource reruns its source whenever a signal
    // read inside it changes.
    let refresh = RwSignal::new(0u32);

    let detail = LocalResource::new(move || {
        refresh.get();
        let api = api.clone();
        async move {
            match account_id() {
                Some(id) => Some(api.get_loan_account(id).await),
                None => None,
            }
        }
    });

    view! {
        <Suspense fallback=|| view! { <LoadingState label="Loading loan account…" /> }>
            {move || {
                detail
                    .get()
                    .map(|wrapped| wrapped.take())
                    .flatten()
                    .map(|result| match result {
                        Ok(d) => view! { <LoanAccountContent detail=d on_payment_recorded=move || refresh.update(|n| *n += 1) /> }.into_any(),
                        Err(e) => view! { <ErrorAlert message=format!("Couldn't load this loan account: {e}") /> }.into_any(),
                    })
            }}
        </Suspense>
    }
}

#[component]
fn LoanAccountContent(
    detail: LoanAccountDetail,
    on_payment_recorded: impl Fn() + Clone + 'static,
) -> impl IntoView {
    let currency = use_currency();
    let account = detail.account.clone();
    let project_href = format!("/projects/{}", detail.project_id);
    let customer_href = format!("/customers/{}", detail.customer_id);

    view! {
        <div class="page-header">
            <div>
                <h1>{account.account_number.clone()}</h1>
                <p>
                    <A href=customer_href.clone()>{detail.customer_name.clone()}</A>
                    " · "
                    <A href=project_href.clone()>{detail.project_name.clone()}</A>
                    " · Plot " {detail.plot_number.clone()}
                </p>
            </div>
            <StatusBadge label=detail.status_label.clone() color=detail.status_color.clone() />
        </div>

        <div class="stat-grid">
            <StatCard label="Principal" value=format_money(account.principal, &currency.get()) />
            <StatCard
                label="Deposit"
                value=format_money(account.deposit_paid, &currency.get())
                sub=format!("of {} required", format_money(account.deposit_required, &currency.get()))
            />
            <StatCard
                label="Instalment"
                value=format_money(account.instalment_amount, &currency.get())
                sub=format!("every {} days", account.repayment_frequency_days)
            />
            <StatCard label="Amount paid" value=format_money(account.amount_paid, &currency.get()) />
            <StatCard
                label="Outstanding balance"
                value=format_money(account.outstanding_balance, &currency.get())
                sub=account.interest_rate.map(|r| format!("{r}% interest")).unwrap_or_else(|| "Interest-free".to_string())
            />
        </div>

        <div class="card">
            <RecordPaymentForm loan_account_id=account.id on_recorded=on_payment_recorded />
        </div>

        <h2 style="margin-top: var(--space-5)">"Payment history"</h2>
        {if detail.payments.is_empty() {
            view! { <p class="text-muted">"No payments recorded yet."</p> }.into_any()
        } else {
            view! {
                <div class="card" style="padding: 0; overflow-x: auto;">
                    <table style="width: 100%; border-collapse: collapse;">
                        <thead>
                            <tr style="text-align: left; border-bottom: 1px solid var(--color-border);">
                                <th style="padding: var(--space-3)">"Date"</th>
                                <th style="padding: var(--space-3)">"Method"</th>
                                <th style="padding: var(--space-3)">"Amount"</th>
                                <th style="padding: var(--space-3)">"Status"</th>
                            </tr>
                        </thead>
                        <tbody>
                            {detail.payments.into_iter().map(|p| view! {
                                <tr style="border-bottom: 1px solid var(--color-border);">
                                    <td style="padding: var(--space-3)">{p.payment_date.to_string()}</td>
                                    <td style="padding: var(--space-3)">{p.method}</td>
                                    <td style="padding: var(--space-3)">{format_money(p.amount, &currency.get())}</td>
                                    <td style="padding: var(--space-3)">{format_payment_status(p.status)}</td>
                                </tr>
                            }).collect_view()}
                        </tbody>
                    </table>
                </div>
            }
                .into_any()
        }}
    }
}

#[component]
fn RecordPaymentForm(loan_account_id: Uuid, on_recorded: impl Fn() + Clone + 'static) -> impl IntoView {
    let api = use_api();
    let currency = use_currency();

    let amount = RwSignal::new(String::new());
    let method = RwSignal::new("M-Pesa".to_string());
    let date = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);
    let submitting = RwSignal::new(false);

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if submitting.get() {
            return;
        }
        error.set(None);

        let Ok(parsed_amount) = Decimal::from_str(amount.get().trim()) else {
            error.set(Some("Enter a valid amount.".to_string()));
            return;
        };
        let payment_date = if date.get().is_empty() {
            chrono::Local::now().date_naive()
        } else {
            match chrono::NaiveDate::parse_from_str(date.get().trim(), "%Y-%m-%d") {
                Ok(d) => d,
                Err(_) => {
                    error.set(Some("Enter a valid date (YYYY-MM-DD).".to_string()));
                    return;
                }
            }
        };

        submitting.set(true);
        let api = api.clone();
        let on_recorded = on_recorded.clone();
        let method_value = method.get();
        spawn_local(async move {
            let result = api
                .record_payment(RecordPaymentInput {
                    loan_account_id,
                    amount: parsed_amount,
                    payment_date,
                    method: method_value,
                })
                .await;
            match result {
                Ok(_) => {
                    amount.set(String::new());
                    date.set(String::new());
                    on_recorded();
                }
                Err(e) => error.set(Some(format!("{e}"))),
            }
            submitting.set(false);
        });
    };

    view! {
        <form on:submit=on_submit>
            <h3 class="mt-0">"Record a payment"</h3>
            {move || error.get().map(|msg| view! { <ErrorAlert message=msg /> })}

            <div class="field">
                <label for="amount">"Amount (" {move || currency.get()} ")"</label>
                <input
                    id="amount"
                    type="text"
                    inputmode="numeric"
                    required
                    prop:value=amount
                    on:input=move |ev| amount.set(event_target_value(&ev))
                />
            </div>

            <div class="field">
                <label for="method">"Payment method"</label>
                <select id="method" prop:value=method on:change=move |ev| method.set(event_target_value(&ev))>
                    <option value="M-Pesa">"M-Pesa"</option>
                    <option value="Bank Transfer">"Bank Transfer"</option>
                    <option value="Cash">"Cash"</option>
                    <option value="Cheque">"Cheque"</option>
                </select>
            </div>

            <div class="field">
                <label for="date">"Date (defaults to today)"</label>
                <input
                    id="date"
                    type="date"
                    prop:value=date
                    on:input=move |ev| date.set(event_target_value(&ev))
                />
            </div>

            <button type="submit" class="btn btn-primary" disabled=submitting>
                {move || if submitting.get() { "Recording…" } else { "Record payment" }}
            </button>
        </form>
    }
}
