//! Settings → Organization / System Configuration — general settings
//! (currency, date format, time zone) and the auto-numbering engine for
//! plots and projects (`GET`/`PUT /api/v1/settings`,
//! `crates/backend/src/routes/settings.rs`). No role check yet: every
//! signed-in org member can reach this page and save changes — see that
//! route module's docs for why, and Phase 2 for what tightens it.

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_location;

use crate::auth::{use_api, use_currency};
use crate::components::{ErrorAlert, LoadingState};
use domain::{NumberingConfigInput, OrganizationSettings, UpdateOrganizationSettingsInput};

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
        if submitting.get() {
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
                    success.set(true);
                }
                Err(e) => error.set(Some(format!("{e}"))),
            }
            submitting.set(false);
        });
    };

    view! {
        <form on:submit=on_submit>
            {move || error.get().map(|msg| view! { <ErrorAlert message=msg /> })}
            {move || {
                success.get().then(|| view! { <div class="alert alert-success">"Settings saved."</div> })
            }}

            <div id="general" class="card" style="max-width: 640px; margin-bottom: var(--space-4)">
                <h2 class="mt-0">"General"</h2>
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

            <div id="numbering" class="card" style="max-width: 640px; margin-bottom: var(--space-4)">
                <h2 class="mt-0">"Plot numbering"</h2>
                <p class="meta">"Preview: " <strong>{plot_preview}</strong></p>
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
                    <label for="plot-padding">"Digit padding"</label>
                    <input
                        id="plot-padding"
                        type="text"
                        inputmode="numeric"
                        prop:value=plot_padding
                        on:input=move |ev| plot_padding.set(event_target_value(&ev))
                    />
                </div>
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

            <div class="card" style="max-width: 640px; margin-bottom: var(--space-4)">
                <h2 class="mt-0">"Project numbering"</h2>
                <p class="meta">"Preview: " <strong>{project_preview}</strong></p>
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
                <label class="checkbox-field">
                    <input
                        type="checkbox"
                        prop:checked=project_include_year
                        on:change=move |ev| project_include_year.set(event_target_checked(&ev))
                    />
                    "Include year"
                </label>
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

            <button type="submit" class="btn btn-primary" disabled=submitting>
                {move || if submitting.get() { "Saving…" } else { "Save settings" }}
            </button>
        </form>
    }
}
