use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;

use crate::api::{ApiError, SignupInput};
use crate::auth::use_api;
use crate::components::{ErrorAlert, LoadingState, PasswordField};

#[component]
pub fn Signup() -> impl IntoView {
    let api = use_api();

    let terms = LocalResource::new({
        let api = api.clone();
        move || {
            let api = api.clone();
            async move { api.current_terms().await }
        }
    });

    let organization_name = RwSignal::new(String::new());
    let organization_code = RwSignal::new(String::new());
    let sector = RwSignal::new(String::new());
    let business_location = RwSignal::new(String::new());
    let business_registration_number = RwSignal::new(String::new());
    let number_of_branches = RwSignal::new(String::new());
    let expected_users = RwSignal::new(String::new());
    let preferred_package_code = RwSignal::new(String::new());
    let contact_person_name = RwSignal::new(String::new());
    let admin_full_name = RwSignal::new(String::new());
    let admin_email = RwSignal::new(String::new());
    let admin_mobile = RwSignal::new(String::new());
    let admin_password = RwSignal::new(String::new());
    let confirm_password = RwSignal::new(String::new());
    let terms_accepted = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let submitting = RwSignal::new(false);
    let submitted = RwSignal::new(false);

    let api_for_submit = api.clone();

    view! {
        <div class="auth-screen">
            <div class="auth-card auth-card-wide card">
                <div class="auth-brand">
                    <span class="logo-mark">"R"</span>
                    <span>"Real Estate Manager"</span>
                </div>

                {move || {
                    if submitted.get() {
                        view! {
                            <div>
                                <div class="pending-approval-icon">"⏳"</div>
                                <h1>"Registration received"</h1>
                                <p>
                                    "Your account is awaiting activation. Our team will review your "
                                    "details and you'll be notified as soon as your workspace is approved."
                                </p>
                                <p class="auth-hint">
                                    <A href="/login">"Back to sign in"</A>
                                </p>
                            </div>
                        }.into_any()
                    } else {
                        let api = api_for_submit.clone();
                        let on_submit = move |ev: leptos::ev::SubmitEvent| {
                            ev.prevent_default();
                            if submitting.get() {
                                return;
                            }
                            error.set(None);

                            if admin_password.get() != confirm_password.get() {
                                error.set(Some("Those passwords don't match.".to_string()));
                                return;
                            }
                            if !terms_accepted.get() {
                                error.set(Some("Please read and accept the Terms & Conditions to continue.".to_string()));
                                return;
                            }
                            let Some(Ok(current_terms)) = terms.get().map(|w| w.take()) else {
                                error.set(Some("Terms & Conditions haven't loaded yet — please wait a moment and try again.".to_string()));
                                return;
                            };

                            submitting.set(true);

                            let api = api.clone();
                            let parsed_branches = number_of_branches.get().trim().parse::<i32>().ok();
                            let parsed_users = expected_users.get().trim().parse::<i32>().ok();
                            let reg_number = business_registration_number.get();
                            let package_code = preferred_package_code.get();
                            let input = SignupInput {
                                organization_name: organization_name.get(),
                                organization_code: organization_code.get(),
                                admin_full_name: admin_full_name.get(),
                                admin_email: admin_email.get(),
                                admin_password: admin_password.get(),
                                admin_mobile: admin_mobile.get(),
                                business_registration_number: if reg_number.trim().is_empty() { None } else { Some(reg_number) },
                                sector: sector.get(),
                                business_location: business_location.get(),
                                contact_person_name: contact_person_name.get(),
                                expected_users: parsed_users,
                                number_of_branches: parsed_branches,
                                preferred_package_code: if package_code.is_empty() { None } else { Some(package_code) },
                                terms_version_id: current_terms.id,
                                terms_accepted: true,
                            };

                            spawn_local(async move {
                                match api.signup(input).await {
                                    Ok(_) => submitted.set(true),
                                    Err(ApiError::InvalidCredentials(msg)) => error.set(Some(msg)),
                                    Err(e) => error.set(Some(format!("Something went wrong: {e}"))),
                                }
                                submitting.set(false);
                            });
                        };

                        view! {
                            <div>
                                <h1>"Register your organization"</h1>
                                <p>"Submit your details for review — trial access starts once approved."</p>

                                {move || error.get().map(|msg| view! { <ErrorAlert message=msg /> })}

                                <form on:submit=on_submit>
                                    <div class="form-grid-2">
                                        <div class="field">
                                            <label for="organization_name">"Organization name"</label>
                                            <input
                                                id="organization_name" type="text" required
                                                prop:value=organization_name
                                                on:input=move |ev| organization_name.set(event_target_value(&ev))
                                            />
                                        </div>
                                        <div class="field">
                                            <label for="organization_code">"Organization code"</label>
                                            <input
                                                id="organization_code" type="text" required
                                                placeholder="e.g. ACACIA"
                                                prop:value=organization_code
                                                on:input=move |ev| organization_code.set(event_target_value(&ev))
                                            />
                                        </div>
                                        <div class="field">
                                            <label for="sector">"Sector"</label>
                                            <input
                                                id="sector" type="text" required
                                                placeholder="e.g. Real estate development"
                                                prop:value=sector
                                                on:input=move |ev| sector.set(event_target_value(&ev))
                                            />
                                        </div>
                                        <div class="field">
                                            <label for="business_location">"Business location"</label>
                                            <input
                                                id="business_location" type="text" required
                                                prop:value=business_location
                                                on:input=move |ev| business_location.set(event_target_value(&ev))
                                            />
                                        </div>
                                        <div class="field">
                                            <label for="business_registration_number">"Registration number " <span class="meta">"(optional)"</span></label>
                                            <input
                                                id="business_registration_number" type="text"
                                                prop:value=business_registration_number
                                                on:input=move |ev| business_registration_number.set(event_target_value(&ev))
                                            />
                                        </div>
                                        <div class="field">
                                            <label for="number_of_branches">"Number of branches " <span class="meta">"(optional)"</span></label>
                                            <input
                                                id="number_of_branches" type="number" min="1"
                                                prop:value=number_of_branches
                                                on:input=move |ev| number_of_branches.set(event_target_value(&ev))
                                            />
                                        </div>
                                        <div class="field">
                                            <label for="expected_users">"Expected users " <span class="meta">"(optional)"</span></label>
                                            <input
                                                id="expected_users" type="number" min="1"
                                                prop:value=expected_users
                                                on:input=move |ev| expected_users.set(event_target_value(&ev))
                                            />
                                        </div>
                                        <div class="field">
                                            <label for="preferred_package_code">"Preferred package " <span class="meta">"(optional)"</span></label>
                                            <input
                                                id="preferred_package_code" type="text"
                                                placeholder="e.g. QUARTERLY"
                                                prop:value=preferred_package_code
                                                on:input=move |ev| preferred_package_code.set(event_target_value(&ev))
                                            />
                                        </div>
                                    </div>

                                    <div class="form-grid-2">
                                        <div class="field">
                                            <label for="contact_person_name">"Contact person"</label>
                                            <input
                                                id="contact_person_name" type="text" required
                                                prop:value=contact_person_name
                                                on:input=move |ev| contact_person_name.set(event_target_value(&ev))
                                            />
                                        </div>
                                        <div class="field">
                                            <label for="admin_full_name">"Admin user's name"</label>
                                            <input
                                                id="admin_full_name" type="text" required
                                                prop:value=admin_full_name
                                                on:input=move |ev| admin_full_name.set(event_target_value(&ev))
                                            />
                                        </div>
                                        <div class="field">
                                            <label for="admin_email">"Admin email"</label>
                                            <input
                                                id="admin_email" type="email" autocomplete="username" required
                                                prop:value=admin_email
                                                on:input=move |ev| admin_email.set(event_target_value(&ev))
                                            />
                                        </div>
                                        <div class="field">
                                            <label for="admin_mobile">"Admin mobile"</label>
                                            <input
                                                id="admin_mobile" type="tel" required
                                                prop:value=admin_mobile
                                                on:input=move |ev| admin_mobile.set(event_target_value(&ev))
                                            />
                                        </div>
                                    </div>

                                    <div class="form-grid-2">
                                        <PasswordField
                                            id="admin_password"
                                            label="Password"
                                            value=admin_password
                                            autocomplete="new-password"
                                            minlength=8
                                            show_strength=true
                                        />
                                        <PasswordField
                                            id="confirm_password"
                                            label="Confirm password"
                                            value=confirm_password
                                            autocomplete="new-password"
                                            minlength=8
                                        />
                                    </div>

                                    <div class="field">
                                        <label>"Terms & Conditions"</label>
                                        <Suspense fallback=|| view! { <LoadingState label="Loading terms…" /> }>
                                            {move || {
                                                terms.get().map(|wrapped| wrapped.take()).map(|result| match result {
                                                    Ok(t) => view! {
                                                        <div class="terms-box">{t.body.clone()}</div>
                                                    }.into_any(),
                                                    Err(e) => view! { <ErrorAlert message=format!("Couldn't load Terms & Conditions: {e}") /> }.into_any(),
                                                })
                                            }}
                                        </Suspense>
                                    </div>
                                    <label class="checkbox-field">
                                        <input
                                            type="checkbox"
                                            prop:checked=terms_accepted
                                            on:change=move |ev| terms_accepted.set(event_target_checked(&ev))
                                        />
                                        <span>"I have read and agree to the Terms & Conditions"</span>
                                    </label>

                                    <button type="submit" class="btn btn-primary btn-block" disabled=submitting>
                                        {move || if submitting.get() { "Submitting…" } else { "Submit for review" }}
                                    </button>
                                </form>

                                <p class="auth-hint">
                                    "Already have an account? " <A href="/login">"Sign in"</A>
                                </p>
                            </div>
                        }.into_any()
                    }
                }}
            </div>
        </div>
    }
}
