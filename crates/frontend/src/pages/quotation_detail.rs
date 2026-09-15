use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;
use uuid::Uuid;

use crate::api::ApiError;
use crate::auth::use_api;
use crate::components::{ErrorAlert, LoadingState, StatusBadge};
use crate::format::{format_kes, format_payment_mode};
use domain::QuotationStatus;

/// Free functions taking owned parameters, not closures captured from
/// the component's top level — see the identical note in
/// `pages/platform_organization_detail.rs` and
/// `pages/customer_detail.rs`: each button is built inside `<Suspense>`'s
/// reactive render closure, where moving a pre-built closure into a
/// fresh `on:click` each render hits E0525 (`FnOnce`, not `Fn`).
fn run(
    fut: impl std::future::Future<Output = Result<domain::Quotation, ApiError>> + 'static,
    working: RwSignal<bool>,
    error: RwSignal<Option<String>>,
    refresh: RwSignal<u32>,
) {
    if working.get() {
        return;
    }
    error.set(None);
    working.set(true);
    spawn_local(async move {
        match fut.await {
            Ok(_) => refresh.update(|n| *n += 1),
            Err(ApiError::InvalidCredentials(msg)) => error.set(Some(msg)),
            Err(e) => error.set(Some(format!("{e}"))),
        }
        working.set(false);
    });
}

#[component]
pub fn QuotationDetailPage() -> impl IntoView {
    let api = use_api();
    let params = use_params_map();
    let quotation_id = move || -> Option<Uuid> { params.read().get("id").and_then(|id| Uuid::parse_str(&id).ok()) };

    let working = RwSignal::new(false);
    let action_error = RwSignal::new(None::<String>);
    let refresh = RwSignal::new(0u32);

    let detail = LocalResource::new({
        let api = api.clone();
        move || {
            let api = api.clone();
            refresh.get();
            async move {
                match quotation_id() {
                    Some(id) => Some(api.get_quotation(id).await),
                    None => None,
                }
            }
        }
    });

    view! {
        <Suspense fallback=|| view! { <LoadingState label="Loading quotation…" /> }>
            {move || {
                detail
                    .get()
                    .map(|wrapped| wrapped.take())
                    .flatten()
                    .map(|result| match result {
                        Ok(d) => {
                            let id = d.quotation.id;
                            let api_for_actions = api.clone();
                            let customer_id = d.customer_id;
                            let converted_sale_id = d.quotation.converted_sale_id;

                            view! {
                                <div class="page-header">
                                    <div>
                                        <h1>{d.plot_number.clone()}</h1>
                                        <p>{d.project_name.clone()} " · " {d.customer_name.clone()}</p>
                                    </div>
                                    <StatusBadge label=d.status_label.clone() color=d.status_color.clone() />
                                </div>

                                {move || action_error.get().map(|msg| view! { <ErrorAlert message=msg /> })}

                                {d.below_minimum_price.then(|| view! {
                                    <div class="alert alert-warning">
                                        "This quote is below the plot's minimum price ("
                                        {format_kes(d.minimum_price)}
                                        "). Accepting it will require sign-off — see "
                                        <A href="/approvals">"Approvals"</A>
                                        "."
                                    </div>
                                })}

                                <div class="card" style="max-width: 480px">
                                    <p><strong>"Payment mode: "</strong>{format_payment_mode(d.quotation.payment_mode)}</p>
                                    <p>
                                        <strong>"Quoted price: "</strong>{format_kes(d.quotation.quoted_price)}
                                        " (asking " {format_kes(d.asking_price)} ")"
                                    </p>
                                    <p><strong>"Valid until: "</strong>{d.quotation.valid_until.to_string()}</p>
                                    {d.quotation.notes.clone().map(|n| view! { <p><strong>"Notes: "</strong>{n}</p> })}

                                    <div style="display:flex; gap: var(--space-2); flex-wrap: wrap; margin-top: var(--space-4)">
                                        {(d.quotation.status == QuotationStatus::Draft && !d.is_expired).then(|| {
                                            let api = api_for_actions.clone();
                                            view! {
                                                <button
                                                    class="btn btn-primary"
                                                    disabled=move || working.get()
                                                    on:click=move |_| {
                                                        let api = api.clone();
                                                        run(async move { api.send_quotation(id).await }, working, action_error, refresh)
                                                    }
                                                >
                                                    "Send to customer"
                                                </button>
                                            }
                                        })}
                                        {(d.quotation.status == QuotationStatus::Sent && !d.is_expired).then(|| {
                                            let api = api_for_actions.clone();
                                            view! {
                                                <button
                                                    class="btn btn-primary"
                                                    disabled=move || working.get()
                                                    on:click=move |_| {
                                                        let api = api.clone();
                                                        run(async move { api.accept_quotation(id).await }, working, action_error, refresh)
                                                    }
                                                >
                                                    "Customer accepted"
                                                </button>
                                            }
                                        })}
                                        {(d.quotation.status == QuotationStatus::Sent).then(|| {
                                            let api = api_for_actions.clone();
                                            view! {
                                                <button
                                                    class="btn btn-danger"
                                                    disabled=move || working.get()
                                                    on:click=move |_| {
                                                        let api = api.clone();
                                                        run(async move { api.reject_quotation(id).await }, working, action_error, refresh)
                                                    }
                                                >
                                                    "Customer declined"
                                                </button>
                                            }
                                        })}
                                        {converted_sale_id.map(|_| view! {
                                            <A href=format!("/customers/{customer_id}") attr:class="btn btn-secondary">
                                                "View resulting sale"
                                            </A>
                                        })}
                                    </div>
                                </div>
                            }
                                .into_any()
                        }
                        Err(e) => view! { <ErrorAlert message=format!("Couldn't load this quotation: {e}") /> }
                            .into_any(),
                    })
            }}
        </Suspense>
    }
}
