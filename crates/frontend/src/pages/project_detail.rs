use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;
use rust_decimal::Decimal;
use std::str::FromStr;
use uuid::Uuid;

use crate::api::{status_meta, CreatePlotInput, CreateSaleInput, PlotWithColor};
use crate::auth::use_api;
use crate::components::{EmptyState, ErrorAlert, LoadingState, StatusBadge};
use crate::format::format_kes;
use domain::{PaymentMode, PlotStatus};

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

/// Plots in these states have no sale attached yet — the only ones a new
/// sale/reservation can be started from. Everything else already has an
/// active `PlotSale` (see `plot_sales_one_active_per_plot` in
/// database/migrations/0001_init.sql) and must go through a different
/// workflow (cancellation, restructure, etc. — not built yet) to change.
fn can_start_sale(status: PlotStatus) -> bool {
    matches!(
        status,
        PlotStatus::Available | PlotStatus::Selected | PlotStatus::TemporarilyHeld
    )
}

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
    let show_add_plot = RwSignal::new(false);

    view! {
        <Suspense fallback=|| view! { <LoadingState label="Loading project…" /> }>
            {move || {
                project
                    .get()
                    .map(|wrapped| wrapped.take())
                    .flatten()
                    .map(|result| match result {
                        Ok(p) => {
                            let project_id_val = p.id;
                            view! {
                                <div class="page-header">
                                    <div>
                                        <h1>{p.name.clone()}</h1>
                                        <p>{p.location.clone()} " · " {p.code.clone()}</p>
                                    </div>
                                    <button
                                        class="btn btn-secondary"
                                        on:click=move |_| show_add_plot.update(|v| *v = !*v)
                                    >
                                        {move || if show_add_plot.get() { "Cancel" } else { "+ Add plot" }}
                                    </button>
                                </div>

                                <Show when=move || show_add_plot.get()>
                                    <div class="card" style="margin-bottom: var(--space-4)">
                                        <AddPlotForm
                                            project_id=project_id_val
                                            on_added=move || {
                                                plots.refetch();
                                                show_add_plot.set(false);
                                            }
                                        />
                                    </div>
                                </Show>
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
                    let plot_id = pwc.plot.id;
                    let asking_price = pwc.plot.asking_price;
                    let startable = can_start_sale(pwc.plot.status);
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

                            <Show when=move || startable>
                                <ReserveForm
                                    plot_id=plot_id
                                    asking_price=asking_price
                                    on_reserved=move || {
                                        plots.refetch();
                                        selected.set(None);
                                    }
                                />
                            </Show>

                            <button class="btn btn-secondary" on:click=move |_| selected.set(None)>
                                "Close"
                            </button>
                        </div>
                    }
                })
        }}
    }
}

/// Adds a plot to this project. `plot_number` uniqueness is enforced per
/// project by `create_plot` (docs/05's fix for the legacy system's
/// global-uniqueness bug — docs/02 §3), not globally.
#[component]
fn AddPlotForm(project_id: Uuid, on_added: impl Fn() + Clone + 'static) -> impl IntoView {
    let api = use_api();

    let plot_number = RwSignal::new(String::new());
    let size = RwSignal::new(String::new());
    let asking_price = RwSignal::new(String::new());
    let minimum_price = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);
    let submitting = RwSignal::new(false);

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if submitting.get() {
            return;
        }
        error.set(None);

        let Ok(parsed_size) = Decimal::from_str(size.get().trim()) else {
            error.set(Some("Enter a valid size.".to_string()));
            return;
        };
        let Ok(parsed_asking) = Decimal::from_str(asking_price.get().trim()) else {
            error.set(Some("Enter a valid asking price.".to_string()));
            return;
        };
        let parsed_minimum = if minimum_price.get().trim().is_empty() {
            parsed_asking
        } else {
            match Decimal::from_str(minimum_price.get().trim()) {
                Ok(v) => v,
                Err(_) => {
                    error.set(Some("Enter a valid minimum price.".to_string()));
                    return;
                }
            }
        };

        submitting.set(true);
        let api = api.clone();
        let on_added = on_added.clone();
        let input = CreatePlotInput {
            project_id,
            plot_number: plot_number.get(),
            size: parsed_size,
            asking_price: parsed_asking,
            minimum_price: parsed_minimum,
        };
        spawn_local(async move {
            match api.create_plot(input).await {
                Ok(_) => on_added(),
                Err(e) => {
                    error.set(Some(format!("{e}")));
                    submitting.set(false);
                }
            }
        });
    };

    view! {
        <form on:submit=on_submit>
            <h3 class="mt-0">"Add a plot"</h3>
            {move || error.get().map(|msg| view! { <ErrorAlert message=msg /> })}

            <div class="field">
                <label for="plot-number">"Plot number"</label>
                <input
                    id="plot-number"
                    type="text"
                    required
                    prop:value=plot_number
                    on:input=move |ev| plot_number.set(event_target_value(&ev))
                />
            </div>

            <div class="field">
                <label for="plot-size">"Size (acres)"</label>
                <input
                    id="plot-size"
                    type="text"
                    inputmode="decimal"
                    required
                    prop:value=size
                    on:input=move |ev| size.set(event_target_value(&ev))
                />
            </div>

            <div class="field">
                <label for="asking-price">"Asking price (KES)"</label>
                <input
                    id="asking-price"
                    type="text"
                    inputmode="numeric"
                    required
                    prop:value=asking_price
                    on:input=move |ev| asking_price.set(event_target_value(&ev))
                />
            </div>

            <div class="field">
                <label for="minimum-price">"Minimum price (KES, optional)"</label>
                <input
                    id="minimum-price"
                    type="text"
                    inputmode="numeric"
                    prop:value=minimum_price
                    on:input=move |ev| minimum_price.set(event_target_value(&ev))
                />
            </div>

            <button type="submit" class="btn btn-primary" disabled=submitting>
                {move || if submitting.get() { "Adding…" } else { "Add plot" }}
            </button>
        </form>
    }
}

/// Starts a sale for the selected plot: pick a customer and payment mode,
/// confirm the price. This is the first step of the sales workflow
/// (docs/07) — deposit/tenor/schedule setup for Lipa Pole Pole comes
/// later (docs/08), not part of this form.
#[component]
fn ReserveForm(
    plot_id: Uuid,
    asking_price: Decimal,
    on_reserved: impl Fn() + Clone + 'static,
) -> impl IntoView {
    let api = use_api();

    let customers = LocalResource::new({
        let api = api.clone();
        move || {
            let api = api.clone();
            async move { api.list_customers().await }
        }
    });

    let customer_id = RwSignal::new(String::new());
    let payment_mode = RwSignal::new("full_cash".to_string());
    let price = RwSignal::new(asking_price.to_string());
    let error = RwSignal::new(None::<String>);
    let submitting = RwSignal::new(false);

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if submitting.get() {
            return;
        }
        error.set(None);

        let Ok(customer) = Uuid::parse_str(&customer_id.get()) else {
            error.set(Some("Choose a customer.".to_string()));
            return;
        };
        let Ok(agreed_price) = Decimal::from_str(price.get().trim()) else {
            error.set(Some("Enter a valid price.".to_string()));
            return;
        };
        let mode = match payment_mode.get().as_str() {
            "lpp_free" => PaymentMode::LipaPolePoleInterestFree,
            "lpp_bearing" => PaymentMode::LipaPolePoleInterestBearing,
            _ => PaymentMode::FullCash,
        };

        submitting.set(true);
        let api = api.clone();
        let on_reserved = on_reserved.clone();
        spawn_local(async move {
            let result = api
                .create_sale(CreateSaleInput {
                    plot_id,
                    customer_id: customer,
                    payment_mode: mode,
                    agreed_price,
                })
                .await;
            match result {
                Ok(_) => on_reserved(),
                Err(e) => {
                    error.set(Some(format!("{e}")));
                    submitting.set(false);
                }
            }
        });
    };

    view! {
        <form on:submit=on_submit style="margin: var(--space-4) 0; padding-top: var(--space-4); border-top: 1px solid var(--color-border);">
            <h3>"Reserve this plot"</h3>
            {move || error.get().map(|msg| view! { <ErrorAlert message=msg /> })}

            <div class="field">
                <label for="customer">"Customer"</label>
                <select
                    id="customer"
                    required
                    prop:value=customer_id
                    on:change=move |ev| customer_id.set(event_target_value(&ev))
                >
                    <option value="">"Select a customer…"</option>
                    <Suspense fallback=|| ()>
                        {move || {
                            customers
                                .get()
                                .map(|wrapped| wrapped.take())
                                .map(|result| match result {
                                    Ok(list) => list
                                        .into_iter()
                                        .map(|c| {
                                            let id = c.customer.id.to_string();
                                            view! { <option value=id>{c.customer.full_name}</option> }
                                        })
                                        .collect_view()
                                        .into_any(),
                                    Err(_) => ().into_any(),
                                })
                        }}
                    </Suspense>
                </select>
                <span class="text-muted" style="font-size: 0.8rem;">
                    "Don't see them? "
                    <A href="/customers/new">"Add a new customer"</A>
                    " (you'll need to reserve again after)."
                </span>
            </div>

            <div class="field">
                <label for="payment-mode">"Payment mode"</label>
                <select
                    id="payment-mode"
                    prop:value=payment_mode
                    on:change=move |ev| payment_mode.set(event_target_value(&ev))
                >
                    <option value="full_cash">"Full cash"</option>
                    <option value="lpp_free">"Lipa Pole Pole (interest-free)"</option>
                    <option value="lpp_bearing">"Lipa Pole Pole (interest-bearing)"</option>
                </select>
            </div>

            <div class="field">
                <label for="price">"Agreed price (KES)"</label>
                <input
                    id="price"
                    type="text"
                    inputmode="numeric"
                    required
                    prop:value=price
                    on:input=move |ev| price.set(event_target_value(&ev))
                />
            </div>

            <button type="submit" class="btn btn-primary" disabled=submitting>
                {move || if submitting.get() { "Reserving…" } else { "Reserve for customer" }}
            </button>
        </form>
    }
}
