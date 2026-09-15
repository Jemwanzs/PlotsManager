use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_params_map;
use uuid::Uuid;

use crate::api::ApiClient;
use crate::auth::use_api;
use crate::components::{ErrorAlert, LoadingState, StatusBadge};

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

#[component]
pub fn PlatformOrganizationDetailPage() -> impl IntoView {
    let api = use_api();
    let params = use_params_map();
    let org_id =
        move || -> Option<Uuid> { params.read().get("id").and_then(|id| Uuid::parse_str(&id).ok()) };

    let action_error = RwSignal::new(None::<String>);
    let working = RwSignal::new(false);
    // Bumped after a successful deactivate/reactivate to force the detail
    // resource to refetch — LocalResource has no manual `.refetch()`, so
    // this extra signal in its source tuple is the idiomatic way to
    // trigger one.
    let refresh = RwSignal::new(0u32);

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
                            let (status_label, status_color) = if is_deactivated {
                                ("Deactivated".to_string(), "#dc2626".to_string())
                            } else {
                                ("Active".to_string(), "#16a34a".to_string())
                            };
                            let plan_line = match (d.summary.subscription_status.as_deref(), d.summary.trial_ends_at) {
                                (Some("trialing"), Some(ends)) => {
                                    format!("Trialing — ends {}", ends.format("%b %d, %Y at %H:%M UTC"))
                                }
                                (Some(status), _) => format!("{}{}", status[..1].to_uppercase(), &status[1..]),
                                (None, _) => "No subscription".to_string(),
                            };

                            let tenant_id = d.summary.id;
                            let api_for_click = api.clone();

                            view! {
                                <div class="page-header">
                                    <div>
                                        <h1>{d.summary.name.clone()}</h1>
                                        <p>{d.summary.code.clone()} " · " {plan_line}</p>
                                    </div>
                                    <div style="display:flex; gap: var(--space-2); align-items:center">
                                        <StatusBadge label=status_label color=status_color />
                                        {if is_deactivated {
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
