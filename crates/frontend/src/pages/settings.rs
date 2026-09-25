//! Settings → Organization / System Configuration — general settings
//! (currency, date format, time zone) and the auto-numbering engine for
//! plots and projects (`GET`/`PUT /api/v1/settings`,
//! `crates/backend/src/routes/settings.rs`). Viewing stays open to any
//! signed-in org member; saving requires
//! `settings:manage_organization` (backend-enforced — this page just
//! disables the Save button for anyone who doesn't have it, so they
//! find out before filling the form rather than from a 403 after).

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_location;
use rust_decimal::Decimal;
use std::str::FromStr;

use crate::auth::{has_permission, use_api, use_auth, use_currency};
use crate::components::{ErrorAlert, LoadingState};
use domain::{
    ChargePolicy, FinancePolicy, NumberingConfigInput, OrganizationSettings, RateType,
    UpdateOrganizationSettingsInput, PERM_SETTINGS_MANAGE_ORGANIZATION,
};

#[component]
pub fn Settings() -> impl IntoView {
    let api = use_api();

    let loaded = LocalResource::new({
        let api = api.clone();
        move || {
            let api = api.clone();
            async move { api.get_settings().await }
        }
    });

    view! {
        <div class="page-header">
            <div>
                <h1>"Organization settings"</h1>
                <p>"Currency, general configuration, and auto-numbering — organization-wide."</p>
            </div>
        </div>

        <Suspense fallback=|| view! { <LoadingState label="Loading settings…" /> }>
            {move || {
                loaded
                    .get()
                    .map(|wrapped| wrapped.take())
                    .map(|result| match result {
                        Ok(s) => view! { <SettingsForm initial=s /> }.into_any(),
                        Err(e) => {
                            view! { <ErrorAlert message=format!("Couldn't load settings: {e}") /> }
                                .into_any()
                        }
                    })
            }}
        </Suspense>
    }
}

#[component]
fn SettingsForm(initial: OrganizationSettings) -> impl IntoView {
    let api = use_api();
    // The signed-in session's currency (`use_currency()`, read by every
    // money-formatting call site app-wide) is separate reactive state
    // from this form's own `currency` signal below — a successful save
    // pushes the new value into it so the rest of the app picks it up
    // immediately, without requiring a re-login.
    let global_currency = use_currency();
    let auth = use_auth();
    let can_manage = move || has_permission(auth, PERM_SETTINGS_MANAGE_ORGANIZATION);

    // `/settings#numbering` (the sidebar's "Numbering configuration" nav
    // sub-item) scrolls straight to that section instead of dropping the
    // visitor at the top of a long page they then have to hunt through.
    let location = use_location();
    Effect::new(move |_| {
        let hash = location.hash.get();
        let id = hash.trim_start_matches('#');
        if !id.is_empty() {
            if let Some(el) = document().get_element_by_id(id) {
                el.scroll_into_view();
            }
        }
    });

    let currency = RwSignal::new(initial.currency.clone());
    let date_format = RwSignal::new(initial.date_format.clone());
    let timezone = RwSignal::new(initial.timezone.clone());

    let plot_prefix = RwSignal::new(initial.plot_numbering.prefix.clone());
    let plot_include_year = RwSignal::new(initial.plot_numbering.include_year);
    let plot_include_code = RwSignal::new(initial.plot_numbering.include_entity_code);
    let plot_padding = RwSignal::new(initial.plot_numbering.padding.to_string());
    let plot_next = RwSignal::new(initial.plot_numbering.next_number.to_string());

    let project_prefix = RwSignal::new(initial.project_numbering.prefix.clone());
    let project_include_year = RwSignal::new(initial.project_numbering.include_year);
    let project_padding = RwSignal::new(initial.project_numbering.padding.to_string());
    let project_next = RwSignal::new(initial.project_numbering.next_number.to_string());

    let alloc_order = &initial.finance_policy.allocation_order;
    let alloc_1 = RwSignal::new(alloc_order.first().cloned().unwrap_or_else(|| "penalty".to_string()));
    let alloc_2 = RwSignal::new(alloc_order.get(1).cloned().unwrap_or_else(|| "interest".to_string()));
    let alloc_3 = RwSignal::new(alloc_order.get(2).cloned().unwrap_or_else(|| "principal".to_string()));
    let grace_period = RwSignal::new(initial.finance_policy.grace_period_days.to_string());
    let interest_enabled = RwSignal::new(initial.finance_policy.interest.enabled);
    let interest_rate_type = RwSignal::new(rate_type_str(initial.finance_policy.interest.rate_type).to_string());
    let interest_rate_value = RwSignal::new(initial.finance_policy.interest.rate_value.to_string());
    let penalty_enabled = RwSignal::new(initial.finance_policy.penalty.enabled);
    let penalty_rate_type = RwSignal::new(rate_type_str(initial.finance_policy.penalty.rate_type).to_string());
    let penalty_rate_value = RwSignal::new(initial.finance_policy.penalty.rate_value.to_string());
    let commission_rate_value = RwSignal::new(initial.default_commission_rate_percent.to_string());

    let error = RwSignal::new(None::<String>);
    let success = RwSignal::new(false);
    let submitting = RwSignal::new(false);

    // Computed client-side from the form's own (possibly unsaved) state
    // via the same `domain::format_sequence_number` the backend uses for
    // its own preview field — no round trip per keystroke. "ABC" stands
    // in for a real project code the same way the backend's own preview
    // does (see `crates/backend/src/routes/settings.rs`).
    let plot_preview = move || {
        let padding = plot_padding.get().trim().parse::<u32>().unwrap_or(4).clamp(1, 10);
        let next = plot_next.get().trim().parse::<u32>().unwrap_or(1);
        let code = plot_include_code.get().then_some("ABC");
        domain::format_sequence_number(&plot_prefix.get(), plot_include_year.get(), code, padding, next)
    };
    let project_preview = move || {
        let padding = project_padding.get().trim().parse::<u32>().unwrap_or(4).clamp(1, 10);
        let next = project_next.get().trim().parse::<u32>().unwrap_or(1);
        domain::format_sequence_number(&project_prefix.get(), project_include_year.get(), None, padding, next)
    };

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if submitting.get() || !can_manage() {
            return;
        }
        error.set(None);
        success.set(false);

        let Ok(plot_padding_val) = plot_padding.get().trim().parse::<u32>() else {
            error.set(Some("Enter a valid plot numbering digit padding.".to_string()));
            return;
        };
        let Ok(plot_next_val) = plot_next.get().trim().parse::<u32>() else {
            error.set(Some("Enter a valid plot numbering starting number.".to_string()));
            return;
        };
        let Ok(project_padding_val) = project_padding.get().trim().parse::<u32>() else {
            error.set(Some("Enter a valid project numbering digit padding.".to_string()));
            return;
        };
        let Ok(project_next_val) = project_next.get().trim().parse::<u32>() else {
            error.set(Some("Enter a valid project numbering starting number.".to_string()));
            return;
        };

        let allocation_order = vec![alloc_1.get(), alloc_2.get(), alloc_3.get()];
        let mut sorted_order = allocation_order.clone();
        sorted_order.sort();
        if sorted_order != ["interest", "penalty", "principal"] {
            error.set(Some("Allocation order must list penalty, interest, and principal, each exactly once.".to_string()));
            return;
        }
        let Ok(grace_period_val) = grace_period.get().trim().parse::<i32>() else {
            error.set(Some("Enter a valid grace period in days.".to_string()));
            return;
        };
        if !(0..=365).contains(&grace_period_val) {
            error.set(Some("Grace period must be between 0 and 365 days.".to_string()));
            return;
        }
        let Ok(interest_rate_val) = Decimal::from_str(interest_rate_value.get().trim()) else {
            error.set(Some("Enter a valid interest rate.".to_string()));
            return;
        };
        let Ok(penalty_rate_val) = Decimal::from_str(penalty_rate_value.get().trim()) else {
            error.set(Some("Enter a valid penalty rate.".to_string()));
            return;
        };
        if interest_rate_val < Decimal::ZERO || penalty_rate_val < Decimal::ZERO {
            error.set(Some("Rates can't be negative.".to_string()));
            return;
        }
        let Ok(commission_rate_val) = Decimal::from_str(commission_rate_value.get().trim()) else {
            error.set(Some("Enter a valid default commission rate.".to_string()));
            return;
        };
        if commission_rate_val < Decimal::ZERO || commission_rate_val > Decimal::from(100) {
            error.set(Some("Default commission rate must be between 0 and 100%.".to_string()));
            return;
        }

        submitting.set(true);
        let api = api.clone();
        let input = UpdateOrganizationSettingsInput {
            currency: currency.get(),
            date_format: date_format.get(),
            timezone: timezone.get(),
            plot_numbering: NumberingConfigInput {
                prefix: plot_prefix.get(),
                include_year: plot_include_year.get(),
                include_entity_code: plot_include_code.get(),
                padding: plot_padding_val,
                next_number: plot_next_val,
            },
            project_numbering: NumberingConfigInput {
                prefix: project_prefix.get(),
                include_year: project_include_year.get(),
                include_entity_code: false,
                padding: project_padding_val,
                next_number: project_next_val,
            },
            finance_policy: FinancePolicy {
                allocation_order,
                grace_period_days: grace_period_val,
                interest: ChargePolicy {
                    enabled: interest_enabled.get(),
                    rate_type: parse_rate_type(&interest_rate_type.get()),
                    rate_value: interest_rate_val,
                },
                penalty: ChargePolicy {
                    enabled: penalty_enabled.get(),
                    rate_type: parse_rate_type(&penalty_rate_type.get()),
                    rate_value: penalty_rate_val,
                },
            },
            default_commission_rate_percent: commission_rate_val,
        };
        spawn_local(async move {
            match api.update_settings(input).await {
                Ok(s) => {
                    global_currency.set(s.currency.clone());
                    currency.set(s.currency);
                    date_format.set(s.date_format);
                    timezone.set(s.timezone);
                    plot_prefix.set(s.plot_numbering.prefix);
                    plot_include_year.set(s.plot_numbering.include_year);
                    plot_include_code.set(s.plot_numbering.include_entity_code);
                    plot_padding.set(s.plot_numbering.padding.to_string());
                    plot_next.set(s.plot_numbering.next_number.to_string());
                    project_prefix.set(s.project_numbering.prefix);
                    project_include_year.set(s.project_numbering.include_year);
                    project_padding.set(s.project_numbering.padding.to_string());
                    project_next.set(s.project_numbering.next_number.to_string());
                    alloc_1.set(s.finance_policy.allocation_order.first().cloned().unwrap_or_else(|| "penalty".to_string()));
                    alloc_2.set(s.finance_policy.allocation_order.get(1).cloned().unwrap_or_else(|| "interest".to_string()));
                    alloc_3.set(s.finance_policy.allocation_order.get(2).cloned().unwrap_or_else(|| "principal".to_string()));
                    grace_period.set(s.finance_policy.grace_period_days.to_string());
                    interest_enabled.set(s.finance_policy.interest.enabled);
                    interest_rate_type.set(rate_type_str(s.finance_policy.interest.rate_type).to_string());
                    interest_rate_value.set(s.finance_policy.interest.rate_value.to_string());
                    penalty_enabled.set(s.finance_policy.penalty.enabled);
                    penalty_rate_type.set(rate_type_str(s.finance_policy.penalty.rate_type).to_string());
                    penalty_rate_value.set(s.finance_policy.penalty.rate_value.to_string());
                    commission_rate_value.set(s.default_commission_rate_percent.to_string());
                    success.set(true);
                }
                Err(e) => error.set(Some(format!("{e}"))),
            }
            submitting.set(false);
        });
    };

    view! {
        <form on:submit=on_submit>
            <Show when=move || !can_manage()>
                <div class="alert alert-warning">"You don't have permission to change organization settings — ask an admin."</div>
            </Show>
            {move || error.get().map(|msg| view! { <ErrorAlert message=msg /> })}
            {move || {
                success.get().then(|| view! { <div class="alert alert-success">"Settings saved."</div> })
            }}

            <div class="section-grid-2">
                <div id="general" class="card span-full">
                    <h2 class="mt-0">"General"</h2>
                    <div class="form-grid-2">
                        <div class="field">
                            <label for="currency">"Currency code"</label>
                            <input
                                id="currency"
                                type="text"
                                required
                                maxlength="5"
                                placeholder="e.g. KES, USD"
                                prop:value=currency
                                on:input=move |ev| currency.set(event_target_value(&ev).to_uppercase())
                            />
                        </div>
                        <div class="field">
                            <label for="date-format">"Date format"</label>
                            <select
                                id="date-format"
                                prop:value=date_format
                                on:change=move |ev| date_format.set(event_target_value(&ev))
                            >
                                <option value="DD/MM/YYYY">"DD/MM/YYYY"</option>
                                <option value="MM/DD/YYYY">"MM/DD/YYYY"</option>
                                <option value="YYYY-MM-DD">"YYYY-MM-DD"</option>
                            </select>
                        </div>
                        <div class="field">
                            <label for="timezone">"Time zone"</label>
                            <input
                                id="timezone"
                                type="text"
                                required
                                placeholder="e.g. Africa/Nairobi"
                                prop:value=timezone
                                on:input=move |ev| timezone.set(event_target_value(&ev))
                            />
                        </div>
                    </div>
                </div>

                <div id="numbering" class="card">
                    <h2 class="mt-0">"Plot numbering"</h2>
                    <p class="meta">"Preview: " <strong>{plot_preview}</strong></p>
                    <div class="form-grid-2">
                        <div class="field">
                            <label for="plot-prefix">"Prefix"</label>
                            <input
                                id="plot-prefix"
                                type="text"
                                placeholder="e.g. PLT"
                                prop:value=plot_prefix
                                on:input=move |ev| plot_prefix.set(event_target_value(&ev))
                            />
                        </div>
                        <div class="field">
                            <label for="plot-padding">"Digit padding"</label>
                            <input
                                id="plot-padding"
                                type="text"
                                inputmode="numeric"
                                prop:value=plot_padding
                                on:input=move |ev| plot_padding.set(event_target_value(&ev))
                            />
                        </div>
                    </div>
                    <label class="checkbox-field">
                        <input
                            type="checkbox"
                            prop:checked=plot_include_year
                            on:change=move |ev| plot_include_year.set(event_target_checked(&ev))
                        />
                        "Include year"
                    </label>
                    <label class="checkbox-field">
                        <input
                            type="checkbox"
                            prop:checked=plot_include_code
                            on:change=move |ev| plot_include_code.set(event_target_checked(&ev))
                        />
                        "Include project code"
                    </label>
                    <div class="field">
                        <label for="plot-next">"Next number to issue"</label>
                        <input
                            id="plot-next"
                            type="text"
                            inputmode="numeric"
                            prop:value=plot_next
                            on:input=move |ev| plot_next.set(event_target_value(&ev))
                        />
                    </div>
                </div>

                <div class="card">
                    <h2 class="mt-0">"Project numbering"</h2>
                    <p class="meta">"Preview: " <strong>{project_preview}</strong></p>
                    <div class="form-grid-2">
                        <div class="field">
                            <label for="project-prefix">"Prefix"</label>
                            <input
                                id="project-prefix"
                                type="text"
                                placeholder="e.g. PRJ"
                                prop:value=project_prefix
                                on:input=move |ev| project_prefix.set(event_target_value(&ev))
                            />
                        </div>
                        <div class="field">
                            <label for="project-padding">"Digit padding"</label>
                            <input
                                id="project-padding"
                                type="text"
                                inputmode="numeric"
                                prop:value=project_padding
                                on:input=move |ev| project_padding.set(event_target_value(&ev))
                            />
                        </div>
                    </div>
                    <label class="checkbox-field">
                        <input
                            type="checkbox"
                            prop:checked=project_include_year
                            on:change=move |ev| project_include_year.set(event_target_checked(&ev))
                        />
                        "Include year"
                    </label>
                    <div class="field">
                        <label for="project-next">"Next number to issue"</label>
                        <input
                            id="project-next"
                            type="text"
                            inputmode="numeric"
                            prop:value=project_next
                            on:input=move |ev| project_next.set(event_target_value(&ev))
                        />
                    </div>
                </div>

                <div id="finance-policy" class="card span-full">
                    <h2 class="mt-0">"Finance policy"</h2>
                    <p class="meta">
                        "Controls how a payment is allocated across a Lipa Pole Pole account, "
                        "when a schedule instalment counts as overdue, and the suggested rate "
                        "on a manual interest/penalty charge."
                    </p>

                    <h3>"Payment allocation order"</h3>
                    <div class="form-grid-2">
                        <div class="field">
                            <label for="alloc-1">"1st priority"</label>
                            <select id="alloc-1" prop:value=alloc_1 on:change=move |ev| alloc_1.set(event_target_value(&ev))>
                                <option value="penalty">"Penalty"</option>
                                <option value="interest">"Interest"</option>
                                <option value="principal">"Principal"</option>
                            </select>
                        </div>
                        <div class="field">
                            <label for="alloc-2">"2nd priority"</label>
                            <select id="alloc-2" prop:value=alloc_2 on:change=move |ev| alloc_2.set(event_target_value(&ev))>
                                <option value="penalty">"Penalty"</option>
                                <option value="interest">"Interest"</option>
                                <option value="principal">"Principal"</option>
                            </select>
                        </div>
                        <div class="field">
                            <label for="alloc-3">"3rd priority"</label>
                            <select id="alloc-3" prop:value=alloc_3 on:change=move |ev| alloc_3.set(event_target_value(&ev))>
                                <option value="penalty">"Penalty"</option>
                                <option value="interest">"Interest"</option>
                                <option value="principal">"Principal"</option>
                            </select>
                        </div>
                        <div class="field">
                            <label for="grace-period">"Overdue grace period (days)"</label>
                            <input
                                id="grace-period"
                                type="text"
                                inputmode="numeric"
                                prop:value=grace_period
                                on:input=move |ev| grace_period.set(event_target_value(&ev))
                            />
                        </div>
                    </div>

                    <div class="form-grid-2" style="margin-top: var(--space-4);">
                        <div>
                            <h3 class="mt-0">"Interest"</h3>
                            <label class="checkbox-field">
                                <input
                                    type="checkbox"
                                    prop:checked=interest_enabled
                                    on:change=move |ev| interest_enabled.set(event_target_checked(&ev))
                                />
                                "Enabled"
                            </label>
                            <div class="field">
                                <label for="interest-rate-type">"Rate type"</label>
                                <select id="interest-rate-type" prop:value=interest_rate_type on:change=move |ev| interest_rate_type.set(event_target_value(&ev))>
                                    <option value="percentage">"Percentage of outstanding principal"</option>
                                    <option value="fixed">"Fixed amount"</option>
                                </select>
                            </div>
                            <div class="field">
                                <label for="interest-rate-value">"Rate"</label>
                                <input
                                    id="interest-rate-value"
                                    type="text"
                                    inputmode="decimal"
                                    prop:value=interest_rate_value
                                    on:input=move |ev| interest_rate_value.set(event_target_value(&ev))
                                />
                            </div>
                        </div>
                        <div>
                            <h3 class="mt-0">"Penalty"</h3>
                            <label class="checkbox-field">
                                <input
                                    type="checkbox"
                                    prop:checked=penalty_enabled
                                    on:change=move |ev| penalty_enabled.set(event_target_checked(&ev))
                                />
                                "Enabled"
                            </label>
                            <div class="field">
                                <label for="penalty-rate-type">"Rate type"</label>
                                <select id="penalty-rate-type" prop:value=penalty_rate_type on:change=move |ev| penalty_rate_type.set(event_target_value(&ev))>
                                    <option value="percentage">"Percentage of outstanding principal"</option>
                                    <option value="fixed">"Fixed amount"</option>
                                </select>
                            </div>
                            <div class="field">
                                <label for="penalty-rate-value">"Rate"</label>
                                <input
                                    id="penalty-rate-value"
                                    type="text"
                                    inputmode="decimal"
                                    prop:value=penalty_rate_value
                                    on:input=move |ev| penalty_rate_value.set(event_target_value(&ev))
                                />
                            </div>
                        </div>
                    </div>
                    <p class="meta" style="margin-top: var(--space-3);">
                        "No automatic charging runs yet — this only suggests an amount on the "
                        "\"Post a manual charge\" form. A charge is still only ever posted when "
                        "someone explicitly submits it."
                    </p>
                </div>
            </div>

            <div class="card form-card" style="margin-top: var(--space-4)">
                <h2 class="mt-0">"Agent commission"</h2>
                <p class="meta mt-0">
                    "Accrues automatically when a sale is recorded, as a percentage of the agreed "
                    "price — accrual tracking only, not a payout workflow. A project can override "
                    "this default from its own detail page."
                </p>
                <div class="field" style="max-width: 240px;">
                    <label for="default-commission-rate">"Default rate (%)"</label>
                    <input
                        id="default-commission-rate"
                        type="text"
                        inputmode="decimal"
                        prop:value=commission_rate_value
                        on:input=move |ev| commission_rate_value.set(event_target_value(&ev))
                    />
                </div>
            </div>

            <button type="submit" class="btn btn-primary" disabled=move || submitting.get() || !can_manage() style="margin-top: var(--space-4);">
                {move || if submitting.get() { "Saving…" } else { "Save settings" }}
            </button>
        </form>
    }
}

fn rate_type_str(rate_type: RateType) -> &'static str {
    match rate_type {
        RateType::Percentage => "percentage",
        RateType::Fixed => "fixed",
    }
}

fn parse_rate_type(value: &str) -> RateType {
    if value == "fixed" {
        RateType::Fixed
    } else {
        RateType::Percentage
    }
}
