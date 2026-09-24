use chrono::NaiveDate;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;
use uuid::Uuid;

use crate::api::{lead_stage_meta, ApiClient, ApiError, LeadStage, UpdateLeadInput};
use crate::auth::{has_permission, use_api, use_auth, use_currency};
use domain::{CustomerType, UpdateCustomerInput, PERM_CUSTOMERS_EDIT, PERM_CUSTOMERS_LEADS_UPDATE};
use crate::components::{EmptyState, ErrorAlert, LoadingState, StatusBadge};
use crate::format::{format_money, format_payment_mode};

fn customer_type_value(t: CustomerType) -> &'static str {
    match t {
        CustomerType::Individual => "individual",
        CustomerType::Company => "company",
        CustomerType::Joint => "joint",
    }
}

fn customer_type_from_value(value: &str) -> CustomerType {
    match value {
        "company" => CustomerType::Company,
        "joint" => CustomerType::Joint,
        _ => CustomerType::Individual,
    }
}

fn stage_value(stage: LeadStage) -> &'static str {
    match stage {
        LeadStage::New => "new",
        LeadStage::Contacted => "contacted",
        LeadStage::SiteVisit => "site_visit",
        LeadStage::Negotiating => "negotiating",
        LeadStage::Lost => "lost",
    }
}

fn stage_from_value(value: &str) -> LeadStage {
    match value {
        "contacted" => LeadStage::Contacted,
        "site_visit" => LeadStage::SiteVisit,
        "negotiating" => LeadStage::Negotiating,
        "lost" => LeadStage::Lost,
        _ => LeadStage::New,
    }
}

/// A free function rather than a closure captured from the component's
/// top level — see the identical note in
/// `pages/platform_organization_detail.rs`: this is called from inside
/// `<Suspense>`'s reactive render closure, where moving a pre-built
/// closure into a fresh `on:click` each render hits E0525.
fn save_lead_update(
    api: ApiClient,
    id: Uuid,
    stage: LeadStage,
    next_follow_up_at: Option<NaiveDate>,
    notes: Option<String>,
    saving: RwSignal<bool>,
    error: RwSignal<Option<String>>,
    refresh: RwSignal<u32>,
) {
    if saving.get() {
        return;
    }
    error.set(None);
    saving.set(true);
    spawn_local(async move {
        let result = api
            .update_lead(
                id,
                UpdateLeadInput { stage, next_follow_up_at, notes },
            )
            .await;
        match result {
            Ok(_) => refresh.update(|n| *n += 1),
            Err(ApiError::InvalidCredentials(msg)) => error.set(Some(msg)),
            Err(e) => error.set(Some(format!("Couldn't save: {e}"))),
        }
        saving.set(false);
    });
}

/// Same free-function shape as `save_lead_update` above, and for the
/// same reason (E0525 from a closure moved into a fresh `on:click`
/// inside `<Suspense>`'s reactive render).
#[allow(clippy::too_many_arguments)]
fn save_customer_update(
    api: ApiClient,
    id: Uuid,
    input: UpdateCustomerInput,
    saving: RwSignal<bool>,
    error: RwSignal<Option<String>>,
    refresh: RwSignal<u32>,
) {
    if saving.get() {
        return;
    }
    error.set(None);
    saving.set(true);
    spawn_local(async move {
        let result = api.update_customer(id, input).await;
        match result {
            Ok(_) => refresh.update(|n| *n += 1),
            Err(ApiError::InvalidCredentials(msg)) => error.set(Some(msg)),
            Err(e) => error.set(Some(format!("Couldn't save: {e}"))),
        }
        saving.set(false);
    });
}

#[component]
pub fn CustomerDetail() -> impl IntoView {
    let api = use_api();
    let currency = use_currency();
    let params = use_params_map();
    let customer_id = move || -> Option<Uuid> { params.read().get("id").and_then(|id| Uuid::parse_str(&id).ok()) };
    let auth = use_auth();
    let can_update_lead = has_permission(auth, PERM_CUSTOMERS_LEADS_UPDATE);
    let can_edit_customer = has_permission(auth, PERM_CUSTOMERS_EDIT);

    let save_error = RwSignal::new(None::<String>);
    let saving = RwSignal::new(false);
    let refresh = RwSignal::new(0u32);

    let profile_open = RwSignal::new(false);
    let profile_error = RwSignal::new(None::<String>);
    let profile_saving = RwSignal::new(false);

    let detail = LocalResource::new({
        let api = api.clone();
        move || {
            let api = api.clone();
            refresh.get();
            async move {
                match customer_id() {
                    Some(id) => Some(api.get_customer(id).await),
                    None => None,
                }
            }
        }
    });

    view! {
        <Suspense fallback=|| view! { <LoadingState label="Loading customer…" /> }>
            {move || {
                detail
                    .get()
                    .map(|wrapped| wrapped.take())
                    .flatten()
                    .map(|result| match result {
                        Ok(d) => {
                            let converted = !d.sales.is_empty();
                            let (stage_label, stage_color) = if converted {
                                ("Converted", "#15734f")
                            } else {
                                lead_stage_meta(d.customer.stage)
                            };
                            let customer_id = d.customer.id;
                            let api_for_save = api.clone();

                            let stage_signal = RwSignal::new(d.customer.stage);
                            let follow_up_signal = RwSignal::new(
                                d.customer.next_follow_up_at.map(|dt| dt.to_string()).unwrap_or_default(),
                            );
                            let notes_signal = RwSignal::new(d.customer.notes.clone().unwrap_or_default());

                            let api_for_profile = api.clone();
                            let p_full_name = RwSignal::new(d.customer.full_name.clone());
                            let p_email = RwSignal::new(d.customer.email.clone().unwrap_or_default());
                            let p_phone = RwSignal::new(d.customer.phone.clone().unwrap_or_default());
                            let p_id_number = RwSignal::new(d.customer.id_number.clone().unwrap_or_default());
                            let p_title = RwSignal::new(d.customer.title.clone().unwrap_or_default());
                            let p_customer_type = RwSignal::new(customer_type_value(d.customer.customer_type).to_string());
                            let p_kra_pin = RwSignal::new(d.customer.kra_pin.clone().unwrap_or_default());
                            let p_postal_address = RwSignal::new(d.customer.postal_address.clone().unwrap_or_default());
                            let p_city = RwSignal::new(d.customer.city.clone().unwrap_or_default());
                            let p_physical_address = RwSignal::new(d.customer.physical_address.clone().unwrap_or_default());
                            let p_legacy_number = RwSignal::new(d.customer.legacy_customer_number.clone().unwrap_or_default());
                            let p_nok_name = RwSignal::new(d.customer.next_of_kin_name.clone().unwrap_or_default());
                            let p_nok_relationship = RwSignal::new(d.customer.next_of_kin_relationship.clone().unwrap_or_default());
                            let p_nok_mobile = RwSignal::new(d.customer.next_of_kin_mobile.clone().unwrap_or_default());
                            let p_nok_id_number = RwSignal::new(d.customer.next_of_kin_id_number.clone().unwrap_or_default());
                            let p_nok_address = RwSignal::new(d.customer.next_of_kin_address.clone().unwrap_or_default());

                            view! {
                                <div class="page-header">
                                    <div>
                                        <h1>{d.customer.full_name.clone()}</h1>
                                        <p>
                                            {d.customer.phone.clone().unwrap_or_else(|| "No phone on file".to_string())}
                                            " · "
                                            {d.customer.email.clone().unwrap_or_else(|| "No email on file".to_string())}
                                            {d.customer.id_number.clone().map(|id| format!(" · ID {id}")).unwrap_or_default()}
                                        </p>
                                    </div>
                                    <StatusBadge label=stage_label.to_string() color=stage_color.to_string() />
                                </div>

                                <div class="card form-card" style="margin-bottom: var(--space-5)">
                                    <div class="page-header" style="margin-bottom: var(--space-3)">
                                        <h2 class="mt-0">"Profile"</h2>
                                        {can_edit_customer.then(|| {
                                            view! {
                                                <button
                                                    type="button"
                                                    class="btn btn-secondary"
                                                    on:click=move |_| profile_open.update(|v| *v = !*v)
                                                >
                                                    {move || if profile_open.get() { "Cancel" } else { "Edit profile" }}
                                                </button>
                                            }
                                        })}
                                    </div>
                                    {move || profile_error.get().map(|msg| view! { <ErrorAlert message=msg /> })}
                                    {move || if profile_open.get() {
                                        let api_for_profile = api_for_profile.clone();
                                        view! {
                                            <div class="form-grid-2">
                                                <div class="field">
                                                    <label for="p-full-name">"Full name"</label>
                                                    <input id="p-full-name" type="text" required prop:value=p_full_name on:input=move |ev| p_full_name.set(event_target_value(&ev)) />
                                                </div>
                                                <div class="field">
                                                    <label for="p-title">"Title"</label>
                                                    <input id="p-title" type="text" placeholder="Mr./Mrs./Co." prop:value=p_title on:input=move |ev| p_title.set(event_target_value(&ev)) />
                                                </div>
                                                <div class="field">
                                                    <label for="p-customer-type">"Customer type"</label>
                                                    <select id="p-customer-type" prop:value=p_customer_type on:change=move |ev| p_customer_type.set(event_target_value(&ev))>
                                                        <option value="individual">"Individual"</option>
                                                        <option value="company">"Company"</option>
                                                        <option value="joint">"Joint"</option>
                                                    </select>
                                                </div>
                                                <div class="field">
                                                    <label for="p-email">"Email"</label>
                                                    <input id="p-email" type="email" prop:value=p_email on:input=move |ev| p_email.set(event_target_value(&ev)) />
                                                </div>
                                                <div class="field">
                                                    <label for="p-phone">"Phone"</label>
                                                    <input id="p-phone" type="text" prop:value=p_phone on:input=move |ev| p_phone.set(event_target_value(&ev)) />
                                                </div>
                                                <div class="field">
                                                    <label for="p-id-number">"National ID / Passport"</label>
                                                    <input id="p-id-number" type="text" prop:value=p_id_number on:input=move |ev| p_id_number.set(event_target_value(&ev)) />
                                                </div>
                                                <div class="field">
                                                    <label for="p-kra-pin">"KRA PIN"</label>
                                                    <input id="p-kra-pin" type="text" prop:value=p_kra_pin on:input=move |ev| p_kra_pin.set(event_target_value(&ev)) />
                                                </div>
                                                <div class="field">
                                                    <label for="p-legacy-number">"Legacy customer number"</label>
                                                    <input id="p-legacy-number" type="text" prop:value=p_legacy_number on:input=move |ev| p_legacy_number.set(event_target_value(&ev)) />
                                                </div>
                                                <div class="field">
                                                    <label for="p-postal">"Postal address"</label>
                                                    <input id="p-postal" type="text" prop:value=p_postal_address on:input=move |ev| p_postal_address.set(event_target_value(&ev)) />
                                                </div>
                                                <div class="field">
                                                    <label for="p-city">"City"</label>
                                                    <input id="p-city" type="text" prop:value=p_city on:input=move |ev| p_city.set(event_target_value(&ev)) />
                                                </div>
                                                <div class="field">
                                                    <label for="p-physical">"Physical address"</label>
                                                    <input id="p-physical" type="text" prop:value=p_physical_address on:input=move |ev| p_physical_address.set(event_target_value(&ev)) />
                                                </div>
                                            </div>

                                            <h3>"Next of kin"</h3>
                                            <div class="form-grid-2">
                                                <div class="field">
                                                    <label for="p-nok-name">"Name"</label>
                                                    <input id="p-nok-name" type="text" prop:value=p_nok_name on:input=move |ev| p_nok_name.set(event_target_value(&ev)) />
                                                </div>
                                                <div class="field">
                                                    <label for="p-nok-relationship">"Relationship"</label>
                                                    <input id="p-nok-relationship" type="text" prop:value=p_nok_relationship on:input=move |ev| p_nok_relationship.set(event_target_value(&ev)) />
                                                </div>
                                                <div class="field">
                                                    <label for="p-nok-mobile">"Mobile"</label>
                                                    <input id="p-nok-mobile" type="text" prop:value=p_nok_mobile on:input=move |ev| p_nok_mobile.set(event_target_value(&ev)) />
                                                </div>
                                                <div class="field">
                                                    <label for="p-nok-id">"National ID / Passport"</label>
                                                    <input id="p-nok-id" type="text" prop:value=p_nok_id_number on:input=move |ev| p_nok_id_number.set(event_target_value(&ev)) />
                                                </div>
                                                <div class="field">
                                                    <label for="p-nok-address">"Address"</label>
                                                    <input id="p-nok-address" type="text" prop:value=p_nok_address on:input=move |ev| p_nok_address.set(event_target_value(&ev)) />
                                                </div>
                                            </div>

                                            <button
                                                type="button"
                                                class="btn btn-primary"
                                                disabled=move || profile_saving.get()
                                                on:click=move |_| {
                                                    let non_empty = |s: String| Some(s).filter(|v| !v.trim().is_empty());
                                                    let input = UpdateCustomerInput {
                                                        full_name: p_full_name.get(),
                                                        email: non_empty(p_email.get()),
                                                        phone: non_empty(p_phone.get()),
                                                        id_number: non_empty(p_id_number.get()),
                                                        title: non_empty(p_title.get()),
                                                        customer_type: customer_type_from_value(&p_customer_type.get()),
                                                        kra_pin: non_empty(p_kra_pin.get()),
                                                        postal_address: non_empty(p_postal_address.get()),
                                                        city: non_empty(p_city.get()),
                                                        physical_address: non_empty(p_physical_address.get()),
                                                        legacy_customer_number: non_empty(p_legacy_number.get()),
                                                        next_of_kin_name: non_empty(p_nok_name.get()),
                                                        next_of_kin_relationship: non_empty(p_nok_relationship.get()),
                                                        next_of_kin_mobile: non_empty(p_nok_mobile.get()),
                                                        next_of_kin_id_number: non_empty(p_nok_id_number.get()),
                                                        next_of_kin_address: non_empty(p_nok_address.get()),
                                                    };
                                                    save_customer_update(
                                                        api_for_profile.clone(), customer_id, input,
                                                        profile_saving, profile_error, refresh,
                                                    );
                                                    profile_open.set(false);
                                                }
                                            >
                                                {move || if profile_saving.get() { "Saving…" } else { "Save profile" }}
                                            </button>
                                        }.into_any()
                                    } else {
                                        view! {
                                            <div class="form-grid-2">
                                                <div><span class="meta">"KRA PIN"</span><p class="mt-0">{d.customer.kra_pin.clone().unwrap_or_else(|| "—".to_string())}</p></div>
                                                <div><span class="meta">"City"</span><p class="mt-0">{d.customer.city.clone().unwrap_or_else(|| "—".to_string())}</p></div>
                                                <div><span class="meta">"Postal address"</span><p class="mt-0">{d.customer.postal_address.clone().unwrap_or_else(|| "—".to_string())}</p></div>
                                                <div><span class="meta">"Physical address"</span><p class="mt-0">{d.customer.physical_address.clone().unwrap_or_else(|| "—".to_string())}</p></div>
                                                <div><span class="meta">"Next of kin"</span><p class="mt-0">
                                                    {d.customer.next_of_kin_name.clone().unwrap_or_else(|| "—".to_string())}
                                                    {d.customer.next_of_kin_relationship.clone().map(|r| format!(" ({r})")).unwrap_or_default()}
                                                </p></div>
                                                <div><span class="meta">"Next of kin mobile"</span><p class="mt-0">{d.customer.next_of_kin_mobile.clone().unwrap_or_else(|| "—".to_string())}</p></div>
                                            </div>
                                        }.into_any()
                                    }}
                                </div>

                                {if !converted {
                                    let api = api_for_save.clone();
                                    view! {
                                        <div class="card form-card" style="margin-bottom: var(--space-5)">
                                            <h2 class="mt-0">"Pipeline"</h2>
                                            {d.customer.source.clone().map(|s| view! {
                                                <p class="meta">"Source: " {s}</p>
                                            })}

                                            {move || save_error.get().map(|msg| view! { <ErrorAlert message=msg /> })}

                                            <div class="form-grid-2">
                                                <div class="field">
                                                    <label for="stage">"Stage"</label>
                                                    <select
                                                        id="stage"
                                                        prop:value=move || stage_value(stage_signal.get()).to_string()
                                                        on:change=move |ev| stage_signal.set(stage_from_value(&event_target_value(&ev)))
                                                    >
                                                        <option value="new">"New"</option>
                                                        <option value="contacted">"Contacted"</option>
                                                        <option value="site_visit">"Site Visit"</option>
                                                        <option value="negotiating">"Negotiating"</option>
                                                        <option value="lost">"Lost"</option>
                                                    </select>
                                                </div>

                                                <div class="field">
                                                    <label for="follow_up">"Next follow-up"</label>
                                                    <input
                                                        id="follow_up"
                                                        type="date"
                                                        prop:value=follow_up_signal
                                                        on:input=move |ev| follow_up_signal.set(event_target_value(&ev))
                                                    />
                                                </div>
                                            </div>

                                            <div class="field">
                                                <label for="notes">"Notes"</label>
                                                <textarea
                                                    id="notes"
                                                    prop:value=notes_signal
                                                    on:input=move |ev| notes_signal.set(event_target_value(&ev))
                                                ></textarea>
                                            </div>

                                            {(!can_update_lead).then(|| view! {
                                                <p class="meta">"You don't have permission to update the pipeline stage — ask an admin."</p>
                                            })}
                                            <button
                                                type="button"
                                                class="btn btn-primary"
                                                disabled=move || saving.get() || !can_update_lead
                                                on:click=move |_| {
                                                    let follow_up = NaiveDate::parse_from_str(&follow_up_signal.get(), "%Y-%m-%d").ok();
                                                    let notes = Some(notes_signal.get()).filter(|s| !s.trim().is_empty());
                                                    save_lead_update(
                                                        api.clone(), customer_id, stage_signal.get(), follow_up, notes,
                                                        saving, save_error, refresh,
                                                    );
                                                }
                                            >
                                                {move || if saving.get() { "Saving…" } else { "Save" }}
                                            </button>
                                        </div>
                                    }
                                        .into_any()
                                } else {
                                    view! {}.into_any()
                                }}

                            <h2>"Plots"</h2>
                            {if d.sales.is_empty() {
                                view! {
                                    <EmptyState
                                        icon="\u{1F3D8}\u{FE0F}"
                                        title="No plots yet"
                                        detail="Reserve a plot for this customer from a project's plot map."
                                    />
                                }
                                    .into_any()
                            } else {
                                view! {
                                    <div class="card-grid">
                                        {d.sales
                                            .into_iter()
                                            .map(|sale| {
                                                // A Lipa Pole Pole sale has a loan account — that's the
                                                // more useful destination (payment history/capture)
                                                // than the project. A Full Cash sale has neither yet
                                                // (docs/08 §2.1 payment tracking isn't built), so it
                                                // just links back to the project for now.
                                                let href = sale
                                                    .loan_account_id
                                                    .map(|id| format!("/loan-accounts/{id}"))
                                                    .unwrap_or_else(|| format!("/projects/{}", sale.project_id));
                                                view! {
                                                    <A href=href attr:class="project-card card">
                                                        <div class="page-header" style="margin-bottom: var(--space-2)">
                                                            <h3 class="mt-0">{sale.plot_number.clone()}</h3>
                                                            <StatusBadge label=sale.status_label.clone() color=sale.status_color.clone() />
                                                        </div>
                                                        <div class="meta">{sale.project_name.clone()}</div>
                                                        <p class="mt-0">
                                                            {format_payment_mode(sale.payment_mode)} " · "
                                                            {format_money(sale.agreed_price, &currency.get())}
                                                        </p>
                                                    </A>
                                                }
                                            })
                                            .collect_view()}
                                    </div>
                                }
                                    .into_any()
                            }}
                        }
                            .into_any()
                        }
                        Err(e) => view! { <ErrorAlert message=format!("Couldn't load this customer: {e}") /> }
                            .into_any(),
                    })
            }}
        </Suspense>
    }
}
