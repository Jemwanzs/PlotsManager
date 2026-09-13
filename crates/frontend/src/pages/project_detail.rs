use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use uuid::Uuid;

use crate::api::{status_meta, PlotWithColor};
use crate::auth::use_api;
use crate::components::{EmptyState, ErrorAlert, LoadingState, StatusBadge};
use crate::format::format_kes;
use domain::PlotStatus;

const ALL_STATUSES: &[PlotStatus] = &[
    PlotStatus::Available,
    PlotStatus::Selected,
    PlotStatus::TemporarilyHeld,
    PlotStatus::Reserved,
    PlotStatus::Booked,
    PlotStatus::UnderApproval,
    PlotStatus::Sold,
    PlotStatus::TransferInProgress,
    PlotStatus::Transferred,
    PlotStatus::Blocked,
    PlotStatus::Disputed,
    PlotStatus::Cancelled,
];

#[component]
pub fn ProjectDetail() -> impl IntoView {
    let api = use_api();
    let params = use_params_map();

    let project_id = move || -> Option<Uuid> { params.read().get("id").and_then(|id| Uuid::parse_str(&id).ok()) };

    let api_for_project = api.clone();
    let project = LocalResource::new(move || {
        let api = api_for_project.clone();
        async move {
            match project_id() {
                Some(id) => Some(api.get_project(id).await),
                None => None,
            }
        }
    });

    let plots = LocalResource::new(move || {
        let api = api.clone();
        async move {
            match project_id() {
                Some(id) => Some(api.list_plots(id).await),
                None => None,
            }
        }
    });

    let selected: RwSignal<Option<PlotWithColor>> = RwSignal::new(None);

    view! {
        <Suspense fallback=|| view! { <LoadingState label="Loading project…" /> }>
            {move || {
                project
                    .get()
                    .map(|wrapped| wrapped.take())
                    .flatten()
                    .map(|result| match result {
                        Ok(p) => {
                            view! {
                                <div class="page-header">
                                    <div>
                                        <h1>{p.name.clone()}</h1>
                                        <p>{p.location.clone()} " · " {p.code.clone()}</p>
                                    </div>
                                </div>
                            }
                                .into_any()
                        }
                        Err(e) => view! { <ErrorAlert message=format!("{e}") /> }.into_any(),
                    })
            }}
        </Suspense>

        <div class="legend">
            {ALL_STATUSES
                .iter()
                .map(|status| {
                    let (label, color) = status_meta(*status);
                    view! {
                        <span class="legend-item">
                            <span class="dot" style=format!("background-color: {color}")></span>
                            <span>{label}</span>
                        </span>
                    }
                })
                .collect_view()}
        </div>

        <Suspense fallback=|| view! { <LoadingState label="Loading plots…" /> }>
            {move || {
                plots
                    .get()
                    .map(|wrapped| wrapped.take())
                    .flatten()
                    .map(|result| match result {
                        Ok(list) if list.is_empty() => {
                            view! {
                                <EmptyState
                                    icon="\u{1F5FA}\u{FE0F}"
                                    title="No plots yet"
                                    detail="Plots added to this project will appear here."
                                />
                            }
                                .into_any()
                        }
                        Ok(list) => {
                            view! {
                                <div class="plot-grid">
                                    {list
                                        .into_iter()
                                        .map(|pwc| {
                                            let pwc_for_click = pwc.clone();
                                            view! {
                                                <button
                                                    class="plot-tile"
                                                    style=format!("background-color: {}; border: none; cursor: pointer; text-align: left;", pwc.status_color)
                                                    on:click=move |_| selected.set(Some(pwc_for_click.clone()))
                                                >
                                                    <span class="plot-number">{pwc.plot.plot_number.clone()}</span>
                                                    <span class="plot-price">{format_kes(pwc.plot.asking_price)}</span>
                                                </button>
                                            }
                                        })
                                        .collect_view()}
                                </div>
                            }
                                .into_any()
                        }
                        Err(e) => view! { <ErrorAlert message=format!("Couldn't load plots: {e}") /> }.into_any(),
                    })
            }}
        </Suspense>

        {move || {
            selected
                .get()
                .map(|pwc| {
                    view! {
                        <div class="card" style="margin-top: var(--space-4)">
                            <div class="page-header" style="margin-bottom: var(--space-3)">
                                <h2 class="mt-0">{pwc.plot.plot_number.clone()}</h2>
                                <StatusBadge label=pwc.status_label.clone() color=pwc.status_color.clone() />
                            </div>
                            <p>
                                "Size: " {pwc.plot.size.to_string()} " acres · Asking price: "
                                {format_kes(pwc.plot.asking_price)}
                            </p>
                            {pwc.plot.title_number.clone().map(|t| view! { <p>"Title: " {t}</p> })}
                            <button class="btn btn-secondary" on:click=move |_| selected.set(None)>
                                "Close"
                            </button>
                        </div>
                    }
                })
        }}
    }
}
