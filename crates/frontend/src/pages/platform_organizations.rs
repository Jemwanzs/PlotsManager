use leptos::prelude::*;
use leptos_router::components::A;

use crate::api::PlatformOrganizationSummary;
use crate::auth::use_api;
use crate::components::{EmptyState, ErrorAlert, LoadingState, StatusBadge};

#[component]
pub fn PlatformOrganizations() -> impl IntoView {
    let api = use_api();

    let organizations = LocalResource::new(move || {
        let api = api.clone();
        async move { api.list_platform_organizations().await }
    });

    view! {
        <div class="page-header">
            <div>
                <h1>"Platform"</h1>
                <p>"Every organization on Real Estate Manager — yours to see across, not just your own."</p>
            </div>
        </div>

        <Suspense fallback=|| view! { <LoadingState label="Loading tenants…" /> }>
            {move || {
                organizations
                    .get()
                    .map(|wrapped| wrapped.take())
                    .map(|result| match result {
                        Ok(list) if list.is_empty() => view! {
                            <EmptyState
                                icon="\u{1F3E2}"
                                title="No organizations yet"
                                detail="New tenants show up here as soon as they sign up."
                            />
                        }
                            .into_any(),
                        Ok(list) => view! {
                            <div class="card-grid">
                                {list
                                    .into_iter()
                                    .map(|org| view! { <TenantCard org=org /> })
                                    .collect_view()}
                            </div>
                        }
                            .into_any(),
                        Err(e) => view! { <ErrorAlert message=format!("Couldn't load organizations: {e}") /> }
                            .into_any(),
                    })
            }}
        </Suspense>
    }
}

#[component]
fn TenantCard(org: PlatformOrganizationSummary) -> impl IntoView {
    let href = format!("/platform/{}", org.id);
    let (status_label, status_color) = if org.status == "deactivated" {
        ("Deactivated".to_string(), "#dc2626".to_string())
    } else {
        ("Active".to_string(), "#16a34a".to_string())
    };

    let plan_line = match (org.subscription_status.as_deref(), org.trial_ends_at) {
        (Some("trialing"), Some(ends)) => {
            format!("Trialing — ends {}", ends.format("%b %d, %Y"))
        }
        (Some(status), _) => format!("{}{}", status[..1].to_uppercase(), &status[1..]),
        (None, _) => "No subscription".to_string(),
    };

    view! {
        <A href=href attr:class="project-card card">
            <div class="page-header" style="margin-bottom: var(--space-2)">
                <h3 class="mt-0">{org.name.clone()}</h3>
                <StatusBadge label=status_label color=status_color />
            </div>
            <div class="meta">{org.code.clone()} " · " {org.plan_name.clone().unwrap_or_else(|| "No plan".to_string())}</div>
            <p class="mt-0">
                {plan_line} " · "
                {if org.user_count == 1 { "1 user".to_string() } else { format!("{} users", org.user_count) }}
            </p>
        </A>
    }
}
