use leptos::prelude::*;
use leptos_router::components::A;

use crate::api::ProjectSummary;
use crate::auth::use_api;
use crate::components::{EmptyState, ErrorAlert, LoadingState};

#[component]
pub fn ProjectsList() -> impl IntoView {
    let api = use_api();
    let search = RwSignal::new(String::new());

    let projects = LocalResource::new(move || {
        let api = api.clone();
        async move { api.list_projects().await }
    });

    view! {
        <div class="page-header">
            <div>
                <h1>"Projects"</h1>
                <p>"Every land project your organisation is selling, with live inventory."</p>
            </div>
        </div>

        <input
            class="search-input"
            type="search"
            placeholder="Search by project name or location…"
            prop:value=search
            on:input=move |ev| search.set(event_target_value(&ev))
        />

        <Suspense fallback=|| view! { <LoadingState label="Loading projects…" /> }>
            {move || {
                projects
                    .get()
                    .map(|wrapped| wrapped.take())
                    .map(|result| match result {
                        Ok(list) => {
                            let query = search.get().to_lowercase();
                            let filtered: Vec<ProjectSummary> = list
                                .into_iter()
                                .filter(|p| {
                                    query.is_empty()
                                        || p.name.to_lowercase().contains(&query)
                                        || p.location.to_lowercase().contains(&query)
                                })
                                .collect();

                            if filtered.is_empty() {
                                view! {
                                    <EmptyState
                                        icon="\u{1F50D}"
                                        title="No projects match your search"
                                        detail="Try a different name or location."
                                    />
                                }
                                    .into_any()
                            } else {
                                view! {
                                    <div class="card-grid">
                                        {filtered
                                            .into_iter()
                                            .map(|p| view! { <ProjectCard project=p /> })
                                            .collect_view()}
                                    </div>
                                }
                                    .into_any()
                            }
                        }
                        Err(e) => {
                            view! { <ErrorAlert message=format!("Couldn't load projects: {e}") /> }.into_any()
                        }
                    })
            }}
        </Suspense>
    }
}

#[component]
fn ProjectCard(project: ProjectSummary) -> impl IntoView {
    let percent_available = if project.total_plots > 0 {
        (project.available_plots as f64 / project.total_plots as f64) * 100.0
    } else {
        0.0
    };
    let href = format!("/projects/{}", project.id);

    view! {
        <A href=href attr:class="project-card card">
            <h3>{project.name.clone()}</h3>
            <div class="meta">{project.location.clone()} " · " {project.code.clone()}</div>
            <div class="progress-track">
                <div class="progress-fill" style=format!("width: {percent_available:.0}%")></div>
            </div>
            <div class="progress-label">
                <span>{project.available_plots} " available"</span>
                <span>{project.total_plots} " plots total"</span>
            </div>
        </A>
    }
}
