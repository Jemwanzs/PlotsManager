use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_params_map;
use uuid::Uuid;

use crate::api::ApiClient;
use crate::auth::use_api;
use crate::components::{ErrorAlert, LoadingState, StatusBadge};
use crate::format::organization_status_meta;

/// A free function, not a closure capturing component state, because
/// it's called from inside `<Suspense>`'s reactive render closure
/// (below) — a closure defined at the component's top level and then
/// moved into a `on:click` handler built *inside* another `Fn` closure
/// hits E0525 (`FnOnce`, not `Fn`/`FnMut`): the outer closure only gets
/// `&self` access to what it captured, so it can `.clone()` those
/// captures into a fresh `on:click` handler each render, but it can't
/// move out a whole pre-built closure it captured by value. Taking
/// everything as owned parameters sidesteps that entirely.
fn trigger_status_change(
    api: ApiClient,
    id: Uuid,
    activate: bool,
    action_error: RwSignal<Option<String>>,
    working: RwSignal<bool>,
    refresh: RwSignal<u32>,
) {
    if working.get() {
        return;
    }
    action_error.set(None);
    working.set(true);
    spawn_local(async move {
        let result = if activate {
            api.reactivate_organization(id).await
        } else {
            api.deactivate_organization(id).await
        };
        match result {
            Ok(()) => refresh.update(|n| *n += 1),
            Err(e) => action_error.set(Some(format!("{e}"))),
        }
        working.set(false);
    });
}

/// "Approve & Start Trial" — see `routes/platform.rs::approve_organization`'s
/// own doc comment for why the trial clock starts at approval, not at
/// the original sign-up.
fn trigger_approve(
    api: ApiClient,
    id: Uuid,
    action_error: RwSignal<Option<String>>,
    working: RwSignal<bool>,
    refresh: RwSignal<u32>,
) {
    if working.get() {
        return;
    }
    action_error.set(None);
    working.set(true);
    spawn_local(async move {
        match api.approve_organization(id).await {
            Ok(_) => refresh.update(|n| *n += 1),
            Err(e) => action_error.set(Some(format!("{e}"))),
        }
        working.set(false);
    });
}

fn trigger_reject(
    api: ApiClient,
    id: Uuid,
    reason: String,
    action_error: RwSignal<Option<String>>,
    working: RwSignal<bool>,
    reject_open: RwSignal<bool>,
    refresh: RwSignal<u32>,
) {
    if working.get() {
        return;
    }
    let reason = reason.trim().to_string();
    if reason.is_empty() {
        action_error.set(Some("Enter a reason for rejecting this application.".to_string()));
        return;
    }
    action_error.set(None);
    working.set(true);
    spawn_local(async move {
        match api.reject_organization(id, reason).await {
            Ok(_) => {
                reject_open.set(false);
                refresh.update(|n| *n += 1);
            }
            Err(e) => action_error.set(Some(format!("{e}"))),
        }
        working.set(false);
    });
}

#[component]
pub fn PlatformOrganizationDetailPage() -> impl IntoView {
    let api = use_api();
    let params = use_params_map();
    let org_id =
        move || -> Option<Uuid> { params.read().get("id").and_then(|id| Uuid::parse_str(&id).ok()) };

    let action_error = RwSignal::new(None::<String>);
    let working = RwSignal::new(false);
    // Bumped after a successful deactivate/reactivate/approve/reject to
    // force the detail resource to refetch — LocalResource has no manual
    // `.refetch()`, so this extra signal in its source tuple is the
    // idiomatic way to trigger one.
    let refresh = RwSignal::new(0u32);
    let reject_open = RwSignal::new(false);
    let reject_reason = RwSignal::new(String::new());

    let detail = LocalResource::new({
        let api = api.clone();
        move || {
            let api = api.clone();
            refresh.get();
            async move {
                match org_id() {
                    Some(id) => Some(api.get_platform_organization(id).await),
                    None => None,
                }
            }
        }
    });

    view! {
        <Suspense fallback=|| view! { <LoadingState label="Loading organization…" /> }>
            {move || {
                detail
                    .get()
                    .map(|wrapped| wrapped.take())
                    .flatten()
                    .map(|result| match result {
                        Ok(d) => {
                            let is_deactivated = d.summary.status == "deactivated";
                            let is_pending = d.summary.status == "pending_approval";
                            let (status_label, status_color) = organization_status_meta(&d.summary.status);
                            let plan_line = match (d.summary.subscription_status.as_deref(), d.summary.trial_ends_at) {
                                (Some("trialing"), Some(ends)) => {
                                    format!("Trialing — ends {}", ends.format("%b %d, %Y at %H:%M UTC"))
                                }
                                (Some(status), _) => format!("{}{}", status[..1].to_uppercase(), &status[1..]),
                                (None, _) => "No subscription".to_string(),
                            };

                            let tenant_id = d.summary.id;
                            let api_for_click = api.clone();
                            // The platform owner belongs to exactly one
                            // organization, so a user in this list flagged
                            // `is_platform_owner` means this page *is* that
                            // account's own tenant — deactivating it would
                            // (were the backend not also guarding this
                            // server-side, see `routes/platform.rs::
                            // deactivate_organization`) suspend the one
                            // account that can undo the suspension.
                            let is_own_org = d.users.iter().any(|u| u.is_platform_owner);

                            view! {
                                <div class="page-header">
                                    <div>
                                        <h1>{d.summary.name.clone()}</h1>
                                        <p>{d.summary.code.clone()} " · " {plan_line}</p>
                                    </div>
                                    <div style="display:flex; gap: var(--space-2); align-items:center">
                                        <StatusBadge label=status_label.to_string() color=status_color.to_string() />
                                        {if is_own_org {
                                            view! {
                                                <span class="meta" title="The platform owner's own organization can't be deactivated.">
                                                    "Platform owner's organization"
                                                </span>
                                            }
                                                .into_any()
                                        } else if is_pending {
                                            let api_approve = api_for_click.clone();
                                            let api_reject = api_for_click.clone();
                                            view! {
                                                <button
                                                    class="btn btn-primary"
                                                    disabled=move || working.get()
                                                    on:click=move |_| trigger_approve(
                                                        api_approve.clone(), tenant_id, action_error, working, refresh,
                                                    )
                                                >
                                                    "Approve & Start Trial"
                                                </button>
                                                <button
                                                    class="btn btn-danger"
                                                    disabled=move || working.get()
                                                    on:click=move |_| reject_open.update(|v| *v = !*v)
                                                >
                                                    {move || if reject_open.get() { "Never mind" } else { "Reject" }}
                                                </button>
                                                {move || reject_open.get().then(|| {
                                                    let api_reject = api_reject.clone();
                                                    view! {
                                                        <input
                                                            type="text"
                                                            placeholder="Reason for rejecting"
                                                            style="max-width: 220px;"
                                                            prop:value=reject_reason
                                                            on:input=move |ev| reject_reason.set(event_target_value(&ev))
                                                        />
                                                        <button
                                                            class="btn btn-danger"
                                                            disabled=move || working.get()
                                                            on:click=move |_| trigger_reject(
                                                                api_reject.clone(), tenant_id, reject_reason.get(),
                                                                action_error, working, reject_open, refresh,
                                                            )
                                                        >
                                                            "Confirm rejection"
                                                        </button>
                                                    }
                                                })}
                                            }
                                                .into_any()
                                        } else if is_deactivated {
                                            let api = api_for_click.clone();
                                            view! {
                                                <button
                                                    class="btn btn-primary"
                                                    disabled=move || working.get()
                                                    on:click=move |_| trigger_status_change(
                                                        api.clone(), tenant_id, true, action_error, working, refresh,
                                                    )
                                                >
                                                    "Reactivate"
                                                </button>
                                            }
                                                .into_any()
                                        } else {
                                            let api = api_for_click.clone();
                                            view! {
                                                <button
                                                    class="btn btn-danger"
                                                    disabled=move || working.get()
                                                    on:click=move |_| trigger_status_change(
                                                        api.clone(), tenant_id, false, action_error, working, refresh,
                                                    )
                                                >
                                                    "Deactivate"
                                                </button>
                                            }
                                                .into_any()
                                        }}
                                    </div>
                                </div>

                                {move || action_error.get().map(|msg| view! { <ErrorAlert message=msg /> })}

                                {d.summary.rejected_reason.clone().map(|reason| view! {
                                    <div class="alert alert-warning" style="margin-bottom: var(--space-3)">
                                        <p class="mt-0">
                                            "Rejected"
                                            {d.summary.rejected_at.map(|at| format!(" on {}", at.format("%b %d, %Y"))).unwrap_or_default()}
                                            ": " {reason}
                                        </p>
                                    </div>
                                })}
                                {d.summary.approved_by_name.clone().map(|name| view! {
                                    <p class="meta mt-0" style="margin-bottom: var(--space-3);">
                                        "Approved by " {name}
                                        {d.summary.approved_at.map(|at| format!(" on {}", at.format("%b %d, %Y"))).unwrap_or_default()}
                                    </p>
                                })}

                                <h2>"Users"</h2>
                                <div class="card">
                                    <div class="table-scroll">
                                        <table class="data-table">
                                            <thead>
                                                <tr>
                                                    <th>"Name"</th>
                                                    <th>"Email"</th>
                                                    <th>"Status"</th>
                                                    <th>"Joined"</th>
                                                </tr>
                                            </thead>
                                            <tbody>
                                                {d.users
                                                    .iter()
                                                    .map(|u| {
                                                        view! {
                                                            <tr>
                                                                <td>
                                                                    {u.full_name.clone()}
                                                                    {if u.is_platform_owner {
                                                                        view! { <span class="meta"> " (platform owner)"</span> }.into_any()
                                                                    } else {
                                                                        view! {}.into_any()
                                                                    }}
                                                                </td>
                                                                <td>{u.email.clone()}</td>
                                                                <td>{if u.is_active { "Active" } else { "Inactive" }}</td>
                                                                <td>{u.created_at.format("%b %d, %Y").to_string()}</td>
                                                            </tr>
                                                        }
                                                    })
                                                    .collect_view()}
                                            </tbody>
                                        </table>
                                    </div>
                                </div>

                                <h2>"Access history"</h2>
                                {if d.recent_access.is_empty() {
                                    view! { <p class="meta">"No logins recorded yet."</p> }.into_any()
                                } else {
                                    view! {
                                        <div class="card">
                                            <div class="table-scroll">
                                                <table class="data-table">
                                                    <thead>
                                                        <tr>
                                                            <th>"Who"</th>
                                                            <th>"Action"</th>
                                                            <th>"When"</th>
                                                        </tr>
                                                    </thead>
                                                    <tbody>
                                                        {d.recent_access
                                                            .iter()
                                                            .map(|entry| {
                                                                view! {
                                                                    <tr>
                                                                        <td>{entry.actor_name.clone().unwrap_or_else(|| "Unknown".to_string())}</td>
                                                                        <td>{entry.action.clone()}</td>
                                                                        <td>{entry.created_at.format("%b %d, %Y at %H:%M UTC").to_string()}</td>
                                                                    </tr>
                                                                }
                                                            })
                                                            .collect_view()}
                                                    </tbody>
                                                </table>
                                            </div>
                                        </div>
                                    }
                                        .into_any()
                                }}
                            }
                                .into_any()
                        }
                        Err(e) => view! { <ErrorAlert message=format!("Couldn't load this organization: {e}") /> }
                            .into_any(),
                    })
            }}
        </Suspense>
    }
}
