use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::{ApiClient, ApiError};
use crate::auth::use_api;
use crate::components::{EmptyState, ErrorAlert, LoadingState, StatusBadge};
use crate::format::{format_kes, format_payment_mode};
use domain::ApprovalStatus;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Filter {
    Pending,
    Decided,
    All,
}

impl Filter {
    fn matches(self, status: ApprovalStatus) -> bool {
        match self {
            Filter::All => true,
            Filter::Pending => status == ApprovalStatus::Pending,
            Filter::Decided => status != ApprovalStatus::Pending,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Filter::Pending => "Pending",
            Filter::Decided => "Decided",
            Filter::All => "All",
        }
    }
}

/// A free function taking owned parameters, not a closure captured from
/// the component's top level — see the identical note in
/// `pages/quotation_detail.rs::run`: this is built fresh inside
/// `<Suspense>`'s reactive render closure on every render, where moving
/// a pre-built closure in hits E0525.
fn decide(
    api: ApiClient,
    id: uuid::Uuid,
    approve: bool,
    notes: String,
    working: RwSignal<bool>,
    error: RwSignal<Option<String>>,
    refresh: RwSignal<u32>,
) {
    if working.get() {
        return;
    }
    error.set(None);
    working.set(true);
    let notes = notes.trim();
    let notes = if notes.is_empty() { None } else { Some(notes.to_string()) };
    spawn_local(async move {
        let result = if approve {
            api.approve_request(id, notes).await
        } else {
            api.reject_request(id, notes).await
        };
        match result {
            Ok(_) => refresh.update(|n| *n += 1),
            Err(ApiError::InvalidCredentials(msg)) => error.set(Some(msg)),
            Err(e) => error.set(Some(format!("{e}"))),
        }
        working.set(false);
    });
}

#[component]
pub fn ApprovalsList() -> impl IntoView {
    let api = use_api();
    let filter = RwSignal::new(Filter::Pending);
    let working = RwSignal::new(false);
    let action_error = RwSignal::new(None::<String>);
    let refresh = RwSignal::new(0u32);

    let requests = LocalResource::new({
        let api = api.clone();
        move || {
            let api = api.clone();
            refresh.get();
            async move { api.list_approvals(None).await }
        }
    });

    view! {
        <div class="page-header">
            <div>
                <h1>"Approvals"</h1>
                <p>"Sales and quotation acceptances priced below a plot's minimum — someone other than the requester has to sign off."</p>
            </div>
        </div>

        {move || action_error.get().map(|msg| view! { <ErrorAlert message=msg /> })}

        <div class="filter-tabs">
            {[Filter::Pending, Filter::Decided, Filter::All]
                .into_iter()
                .map(|f| {
                    view! {
                        <button
                            type="button"
                            class="filter-tab"
                            class:active=move || filter.get() == f
                            on:click=move |_| filter.set(f)
                        >
                            {f.label()}
                        </button>
                    }
                })
                .collect_view()}
        </div>

        <Suspense fallback=|| view! { <LoadingState label="Loading approvals…" /> }>
            {move || {
                let api = api.clone();
                requests
                    .get()
                    .map(|wrapped| wrapped.take())
                    .map(|result| match result {
                        Ok(list) => {
                            let visible: Vec<_> = list
                                .into_iter()
                                .filter(|r| filter.get().matches(r.request.status))
                                .collect();
                            if visible.is_empty() {
                                view! {
                                    <EmptyState
                                        icon="\u{2705}"
                                        title="Nothing here"
                                        detail="Below-minimum sales and quotation acceptances will show up here awaiting sign-off."
                                    />
                                }
                                    .into_any()
                            } else {
                                let api = api.clone();
                                view! {
                                    <div class="card-grid">
                                        {visible
                                            .into_iter()
                                            .map(|r| {
                                                let api = api.clone();
                                                let id = r.request.id;
                                                let notes = RwSignal::new(String::new());
                                                let pending = r.request.status == ApprovalStatus::Pending;
                                                view! {
                                                    <div class="card">
                                                        <div class="page-header" style="margin-bottom: var(--space-2)">
                                                            <h3 class="mt-0">{r.plot_number.clone()}</h3>
                                                            <StatusBadge label=r.status_label.clone() color=r.status_color.clone() />
                                                        </div>
                                                        <div class="meta">{r.project_name.clone()} " · " {r.customer_name.clone()}</div>
                                                        <p class="mt-0">
                                                            {format_payment_mode(r.request.payment_mode)} " · "
                                                            {format_kes(r.request.agreed_price)}
                                                            " (minimum " {format_kes(r.request.minimum_price)} ")"
                                                        </p>
                                                        <p class="meta mt-0">{r.request.reason.clone()}</p>
                                                        <p class="meta mt-0">"Requested by " {r.requested_by_name.clone()}</p>
                                                        {r.decided_by_name.clone().map(|name| view! {
                                                            <p class="meta mt-0">
                                                                "Decided by " {name}
                                                                {r.request.decision_notes.clone().map(|n| view! { " — " {n} })}
                                                            </p>
                                                        })}

                                                        {pending.then(|| {
                                                            let api_approve = api.clone();
                                                            let api_reject = api.clone();
                                                            view! {
                                                                <div class="field">
                                                                    <label for=format!("notes-{id}")>"Notes (optional)"</label>
                                                                    <input
                                                                        id=format!("notes-{id}")
                                                                        type="text"
                                                                        prop:value=notes
                                                                        on:input=move |ev| notes.set(event_target_value(&ev))
                                                                    />
                                                                </div>
                                                                <div style="display:flex; gap: var(--space-2); flex-wrap: wrap; margin-top: var(--space-2)">
                                                                    <button
                                                                        class="btn btn-primary"
                                                                        disabled=move || working.get()
                                                                        on:click=move |_| {
                                                                            decide(api_approve.clone(), id, true, notes.get(), working, action_error, refresh)
                                                                        }
                                                                    >
                                                                        "Approve"
                                                                    </button>
                                                                    <button
                                                                        class="btn btn-danger"
                                                                        disabled=move || working.get()
                                                                        on:click=move |_| {
                                                                            decide(api_reject.clone(), id, false, notes.get(), working, action_error, refresh)
                                                                        }
                                                                    >
                                                                        "Reject"
                                                                    </button>
                                                                </div>
                                                            }
                                                        })}
                                                    </div>
                                                }
                                            })
                                            .collect_view()}
                                    </div>
                                }
                                    .into_any()
                            }
                        }
                        Err(e) => view! { <ErrorAlert message=format!("Couldn't load approvals: {e}") /> }
                            .into_any(),
                    })
            }}
        </Suspense>
    }
}
