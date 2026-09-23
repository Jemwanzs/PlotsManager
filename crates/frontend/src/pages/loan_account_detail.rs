use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;
use rust_decimal::Decimal;
use std::str::FromStr;
use uuid::Uuid;

use crate::api::{LoanAccountDetail, RecordPaymentInput};
use crate::auth::{has_permission, use_api, use_auth, use_currency};
use crate::components::{ErrorAlert, LoadingState, StatCard, StatusBadge};
use crate::format::{format_money, format_payment_status};
use domain::{
    ChargeType, PostChargeInput, PostWaiverInput, WaiverType, PERM_FINANCE_POST_CHARGES,
    PERM_FINANCE_REVERSE, PERM_PAYMENTS_RECORD,
};

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
    let auth = use_auth();
    let can_record = has_permission(auth, PERM_PAYMENTS_RECORD);
    let can_post_charges = has_permission(auth, PERM_FINANCE_POST_CHARGES);
    let can_reverse = has_permission(auth, PERM_FINANCE_REVERSE);
    let account = detail.account.clone();
    let project_href = format!("/projects/{}", detail.project_id);
    let customer_href = format!("/customers/{}", detail.customer_id);
    let statement_href = format!("/loan-accounts/{}/statement", account.id);

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
            <div style="display:flex; gap: var(--space-2); align-items: center;">
                <A href=statement_href attr:class="btn btn-secondary">"View Statement"</A>
                <StatusBadge label=detail.status_label.clone() color=detail.status_color.clone() />
            </div>
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
            {account.next_instalment_due_date.map(|due| {
                view! {
                    <StatCard
                        label="Next instalment"
                        value=format_money(account.next_instalment_amount.unwrap_or_default(), &currency.get())
                        sub=format!("due {due}")
                    />
                }
            })}
            {(account.days_in_arrears > 0).then(|| {
                view! {
                    <StatCard label="Days overdue" value=account.days_in_arrears.to_string() />
                }
            })}
        </div>

        <div class="section-grid-2">
            {if can_record {
                view! {
                    <div class="card">
                        <RecordPaymentForm loan_account_id=account.id on_recorded=on_payment_recorded.clone() />
                    </div>
                }.into_any()
            } else {
                view! {
                    <div class="alert alert-warning">"You don't have permission to record payments — ask an admin."</div>
                }.into_any()
            }}
            {can_post_charges.then(|| view! {
                <div class="card">
                    <PostChargeForm loan_account_id=account.id on_posted=on_payment_recorded.clone() />
                </div>
            })}
            {can_reverse.then(|| view! {
                <div class="card">
                    <WaiveForm loan_account_id=account.id on_waived=on_payment_recorded.clone() />
                </div>
            })}
        </div>

        <h2 style="margin-top: var(--space-5)">"Payment history"</h2>
        <p class="text-muted" style="margin-top: calc(var(--space-2) * -1);">
            "To reverse a payment or charge, use the ledger entries on the "
            <A href=format!("/loan-accounts/{}/statement", account.id)>"Statement"</A>
            " page."
        </p>
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
pub(crate) fn ReverseButton(
    loan_account_id: Uuid,
    entry_id: Uuid,
    // `Send + Sync` here (not just the usual `Fn() + Clone + 'static`
    // other on_recorded/on_posted callbacks in this file use) because
    // this component gets used inside a reactive `{move || ...}}`
    // template closure on the Statement page — Leptos's `ReactiveFunction`/
    // `IntoRender` bounds require that even in a single-threaded WASM
    // app, same reason `ApiClient` needs `Arc<Mutex<_>>` (see its own
    // doc comment).
    on_reversed: impl Fn() + Clone + Send + Sync + 'static,
) -> impl IntoView {
    let api = use_api();
    let open = RwSignal::new(false);
    let reason = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);
    let submitting = RwSignal::new(false);

    let confirm = move |_: leptos::ev::MouseEvent| {
        if submitting.get() {
            return;
        }
        let reason_value = reason.get().trim().to_string();
        if reason_value.is_empty() {
            error.set(Some("Enter a reason.".to_string()));
            return;
        }
        error.set(None);
        submitting.set(true);
        let api = api.clone();
        let on_reversed = on_reversed.clone();
        spawn_local(async move {
            let result = api.reverse_entry(loan_account_id, entry_id, reason_value).await;
            match result {
                Ok(_) => {
                    open.set(false);
                    reason.set(String::new());
                    on_reversed();
                }
                Err(e) => error.set(Some(format!("{e}"))),
            }
            submitting.set(false);
        });
    };

    view! {
        {move || {
            if open.get() {
                let confirm = confirm.clone();
                view! {
                    <div style="display:flex; flex-direction:column; gap: var(--space-2);">
                        <div style="display:flex; gap: var(--space-2); align-items:center;">
                            <input
                                type="text"
                                placeholder="Reason"
                                style="max-width: 160px;"
                                prop:value=reason
                                on:input=move |ev| reason.set(event_target_value(&ev))
                            />
                            <button class="btn btn-danger" on:click=confirm disabled=submitting>
                                {move || if submitting.get() { "…" } else { "Confirm" }}
                            </button>
                            <button class="btn btn-secondary" on:click=move |_| open.set(false)>"Cancel"</button>
                        </div>
                        {move || error.get().map(|msg| view! { <ErrorAlert message=msg /> })}
                    </div>
                }.into_any()
            } else {
                view! {
                    <button class="btn btn-secondary" on:click=move |_| open.set(true)>"Reverse"</button>
                }.into_any()
            }
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

    // Recomputes the allocation preview whenever the amount changes —
    // same penalty -> interest -> principal waterfall
    // `record_payment` itself applies server-side, so what's shown
    // here is guaranteed to match what actually gets posted.
    let api_for_preview = api.clone();
    let preview = LocalResource::new(move || {
        let api = api_for_preview.clone();
        let parsed = Decimal::from_str(amount.get().trim()).ok().filter(|a| *a > Decimal::ZERO);
        async move {
            match parsed {
                Some(a) => Some(api.preview_allocation(loan_account_id, a).await),
                None => None,
            }
        }
    });

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

            {move || {
                preview.get().map(|wrapped| wrapped.take()).flatten().and_then(|r| r.ok()).map(|p| view! {
                    <div class="alert alert-info" style="display:flex; flex-direction:column; gap: var(--space-1);">
                        <strong>"Allocation preview"</strong>
                        <span>"Penalty: " {format_money(p.penalty_paid, &currency.get())}</span>
                        <span>"Interest: " {format_money(p.interest_paid, &currency.get())}</span>
                        <span>"Principal: " {format_money(p.principal_paid, &currency.get())}</span>
                        <span>"New balance: " <strong>{format_money(p.new_balance, &currency.get())}</strong></span>
                    </div>
                })
            }}

            <button type="submit" class="btn btn-primary" disabled=submitting>
                {move || if submitting.get() { "Recording…" } else { "Record payment" }}
            </button>
        </form>
    }
}

#[component]
fn PostChargeForm(loan_account_id: Uuid, on_posted: impl Fn() + Clone + 'static) -> impl IntoView {
    let api = use_api();

    let charge_type = RwSignal::new("interest".to_string());
    let amount = RwSignal::new(String::new());
    let charge_date = RwSignal::new(String::new());
    let reason = RwSignal::new(String::new());
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
        if parsed_amount <= Decimal::ZERO {
            error.set(Some("Amount must be greater than zero.".to_string()));
            return;
        }
        let reason_value = reason.get().trim().to_string();
        if reason_value.is_empty() {
            error.set(Some("Enter a reason for this charge.".to_string()));
            return;
        }
        let parsed_date = if charge_date.get().is_empty() {
            chrono::Local::now().date_naive()
        } else {
            match chrono::NaiveDate::parse_from_str(charge_date.get().trim(), "%Y-%m-%d") {
                Ok(d) => d,
                Err(_) => {
                    error.set(Some("Enter a valid date (YYYY-MM-DD).".to_string()));
                    return;
                }
            }
        };
        let charge_type_value = if charge_type.get() == "penalty" {
            ChargeType::Penalty
        } else {
            ChargeType::Interest
        };

        submitting.set(true);
        let api = api.clone();
        let on_posted = on_posted.clone();
        spawn_local(async move {
            let result = api
                .post_charge(PostChargeInput {
                    loan_account_id,
                    charge_type: charge_type_value,
                    amount: parsed_amount,
                    charge_date: parsed_date,
                    reason: reason_value,
                })
                .await;
            match result {
                Ok(_) => {
                    amount.set(String::new());
                    charge_date.set(String::new());
                    reason.set(String::new());
                    on_posted();
                }
                Err(e) => error.set(Some(format!("{e}"))),
            }
            submitting.set(false);
        });
    };

    view! {
        <form on:submit=on_submit>
            <h3 class="mt-0">"Post a manual charge"</h3>
            {move || error.get().map(|msg| view! { <ErrorAlert message=msg /> })}

            <div class="field">
                <label for="charge-type">"Charge type"</label>
                <select id="charge-type" prop:value=charge_type on:change=move |ev| charge_type.set(event_target_value(&ev))>
                    <option value="interest">"Interest"</option>
                    <option value="penalty">"Penalty"</option>
                </select>
            </div>

            <div class="field">
                <label for="charge-amount">"Amount"</label>
                <input
                    id="charge-amount"
                    type="text"
                    inputmode="numeric"
                    required
                    prop:value=amount
                    on:input=move |ev| amount.set(event_target_value(&ev))
                />
            </div>

            <div class="field">
                <label for="charge-date">"Date (defaults to today)"</label>
                <input
                    id="charge-date"
                    type="date"
                    prop:value=charge_date
                    on:input=move |ev| charge_date.set(event_target_value(&ev))
                />
            </div>

            <div class="field">
                <label for="charge-reason">"Reason"</label>
                <input
                    id="charge-reason"
                    type="text"
                    required
                    prop:value=reason
                    on:input=move |ev| reason.set(event_target_value(&ev))
                />
            </div>

            <button type="submit" class="btn btn-secondary" disabled=submitting>
                {move || if submitting.get() { "Posting…" } else { "Post charge" }}
            </button>
        </form>
    }
}

#[component]
fn WaiveForm(loan_account_id: Uuid, on_waived: impl Fn() + Clone + 'static) -> impl IntoView {
    let api = use_api();

    let waiver_type = RwSignal::new("interest".to_string());
    let amount = RwSignal::new(String::new());
    let waiver_date = RwSignal::new(String::new());
    let reason = RwSignal::new(String::new());
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
        if parsed_amount <= Decimal::ZERO {
            error.set(Some("Amount must be greater than zero.".to_string()));
            return;
        }
        let reason_value = reason.get().trim().to_string();
        if reason_value.is_empty() {
            error.set(Some("Enter a reason for this waiver.".to_string()));
            return;
        }
        let parsed_date = if waiver_date.get().is_empty() {
            chrono::Local::now().date_naive()
        } else {
            match chrono::NaiveDate::parse_from_str(waiver_date.get().trim(), "%Y-%m-%d") {
                Ok(d) => d,
                Err(_) => {
                    error.set(Some("Enter a valid date (YYYY-MM-DD).".to_string()));
                    return;
                }
            }
        };
        let waiver_type_value = if waiver_type.get() == "penalty" {
            WaiverType::Penalty
        } else {
            WaiverType::Interest
        };

        submitting.set(true);
        let api = api.clone();
        let on_waived = on_waived.clone();
        spawn_local(async move {
            let result = api
                .post_waiver(PostWaiverInput {
                    loan_account_id,
                    waiver_type: waiver_type_value,
                    amount: parsed_amount,
                    waiver_date: parsed_date,
                    reason: reason_value,
                })
                .await;
            match result {
                Ok(_) => {
                    amount.set(String::new());
                    waiver_date.set(String::new());
                    reason.set(String::new());
                    on_waived();
                }
                Err(e) => error.set(Some(format!("{e}"))),
            }
            submitting.set(false);
        });
    };

    view! {
        <form on:submit=on_submit>
            <h3 class="mt-0">"Waive interest or penalty"</h3>
            {move || error.get().map(|msg| view! { <ErrorAlert message=msg /> })}

            <div class="field">
                <label for="waiver-type">"Waive"</label>
                <select id="waiver-type" prop:value=waiver_type on:change=move |ev| waiver_type.set(event_target_value(&ev))>
                    <option value="interest">"Interest"</option>
                    <option value="penalty">"Penalty"</option>
                </select>
            </div>

            <div class="field">
                <label for="waiver-amount">"Amount"</label>
                <input
                    id="waiver-amount"
                    type="text"
                    inputmode="numeric"
                    required
                    prop:value=amount
                    on:input=move |ev| amount.set(event_target_value(&ev))
                />
            </div>

            <div class="field">
                <label for="waiver-date">"Date (defaults to today)"</label>
                <input
                    id="waiver-date"
                    type="date"
                    prop:value=waiver_date
                    on:input=move |ev| waiver_date.set(event_target_value(&ev))
                />
            </div>

            <div class="field">
                <label for="waiver-reason">"Reason"</label>
                <input
                    id="waiver-reason"
                    type="text"
                    required
                    prop:value=reason
                    on:input=move |ev| reason.set(event_target_value(&ev))
                />
            </div>

            <button type="submit" class="btn btn-secondary" disabled=submitting>
                {move || if submitting.get() { "Waiving…" } else { "Waive" }}
            </button>
        </form>
    }
}
