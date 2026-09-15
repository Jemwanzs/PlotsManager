use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_navigate;

use crate::api::{ApiError, CreateCustomerInput};
use crate::auth::use_api;
use crate::components::ErrorAlert;

#[component]
pub fn NewCustomer() -> impl IntoView {
    let api = use_api();
    let navigate = use_navigate();

    let full_name = RwSignal::new(String::new());
    let phone = RwSignal::new(String::new());
    let email = RwSignal::new(String::new());
    let id_number = RwSignal::new(String::new());
    let source = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);
    let submitting = RwSignal::new(false);

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if submitting.get() {
            return;
        }
        error.set(None);
        submitting.set(true);

        let api = api.clone();
        let navigate = navigate.clone();
        let input = CreateCustomerInput {
            full_name: full_name.get(),
            phone: Some(phone.get()).filter(|s| !s.is_empty()),
            email: Some(email.get()).filter(|s| !s.is_empty()),
            id_number: Some(id_number.get()).filter(|s| !s.is_empty()),
            source: Some(source.get()).filter(|s| !s.is_empty()),
        };

        spawn_local(async move {
            match api.create_customer(input).await {
                Ok(customer) => navigate(&format!("/customers/{}", customer.id), Default::default()),
                Err(ApiError::InvalidCredentials(msg)) => error.set(Some(msg)),
                Err(e) => error.set(Some(format!("Something went wrong: {e}"))),
            }
            submitting.set(false);
        });
    };

    view! {
        <div class="page-header">
            <div>
                <h1>"New customer"</h1>
                <p>"Capture a lead the moment they're interested — enrich the rest later."</p>
            </div>
        </div>

        <div class="card" style="max-width: 480px;">
            <form on:submit=on_submit>
                {move || error.get().map(|msg| view! { <ErrorAlert message=msg /> })}

                <div class="field">
                    <label for="full_name">"Full name"</label>
                    <input
                        id="full_name"
                        type="text"
                        required
                        prop:value=full_name
                        on:input=move |ev| full_name.set(event_target_value(&ev))
                    />
                </div>

                <div class="field">
                    <label for="phone">"Phone"</label>
                    <input
                        id="phone"
                        type="tel"
                        prop:value=phone
                        on:input=move |ev| phone.set(event_target_value(&ev))
                    />
                </div>

                <div class="field">
                    <label for="email">"Email"</label>
                    <input
                        id="email"
                        type="email"
                        prop:value=email
                        on:input=move |ev| email.set(event_target_value(&ev))
                    />
                </div>

                <div class="field">
                    <label for="id_number">"National ID / Passport"</label>
                    <input
                        id="id_number"
                        type="text"
                        prop:value=id_number
                        on:input=move |ev| id_number.set(event_target_value(&ev))
                    />
                </div>

                <div class="field">
                    <label for="source">"How did they find us?"</label>
                    <select
                        id="source"
                        prop:value=source
                        on:change=move |ev| source.set(event_target_value(&ev))
                    >
                        <option value="">"Not sure yet"</option>
                        <option value="Walk-in">"Walk-in"</option>
                        <option value="Referral">"Referral"</option>
                        <option value="Website">"Website"</option>
                        <option value="Phone inquiry">"Phone inquiry"</option>
                        <option value="Agent outreach">"Agent outreach"</option>
                        <option value="Social media">"Social media"</option>
                    </select>
                </div>

                <button type="submit" class="btn btn-primary" disabled=submitting>
                    {move || if submitting.get() { "Saving…" } else { "Save customer" }}
                </button>
            </form>
        </div>
    }
}
