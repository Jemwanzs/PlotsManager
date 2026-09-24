use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::wasm_bindgen::JsCast;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;
use rust_decimal::Decimal;
use std::str::FromStr;
use uuid::Uuid;

use crate::api::{
    status_meta, CreatePlotInput, CreateQuotationInput, CreateSaleInput, PlotWithColor,
    UpdatePlotInput,
};
use crate::auth::{has_permission, use_api, use_auth, use_currency};
use crate::components::{DocumentsPanel, EmptyState, ErrorAlert, LoadingState, StatusBadge};
use crate::csv_import::{self, ParsedRow};
use crate::format::{format_money, format_payment_mode};
use domain::{
    MapFeature, MapPolygons, PaymentMode, PlotStatus, PERM_PLOTS_BULK_IMPORT, PERM_PLOTS_CREATE,
    PERM_PLOTS_EDIT, PERM_PLOTS_MAP_EDIT_BOUNDARIES, PERM_PLOTS_MAP_LINK, PERM_PLOTS_MAP_UPLOAD,
    PERM_PLOTS_TRANSACTIONS_CREATE, PERM_QUOTES_CREATE,
};

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

/// A plain function, not a closure stored in a `let` — reactive view
/// closures that render the map's polygons need a fresh, repeatable
/// way to look up a plot's colour without capturing (and exhausting)
/// an owned `Vec<PlotWithColor>`; taking `&[PlotWithColor]` by
/// reference each call sidesteps that entirely.
fn feature_color(plots: &[PlotWithColor], plot_id: Option<Uuid>) -> String {
    plot_id
        .and_then(|id| plots.iter().find(|p| p.plot.id == id))
        .map(|p| p.status_color.clone())
        .unwrap_or_else(|| "#9ca3af".to_string())
}

/// Where a shape's map label renders — the plain average of its
/// boundary points. Not a true polygon centroid (that needs an
/// area-weighted formula for a very irregular shape to look centred),
/// but plot boundaries are close enough to convex in practice that the
/// simpler average reads fine, and it's the same approach the "click a
/// point in the middle" mental model already assumes.
fn polygon_centroid(points: &[[f64; 2]]) -> (f64, f64) {
    if points.is_empty() {
        return (0.0, 0.0);
    }
    let (sum_x, sum_y) = points.iter().fold((0.0, 0.0), |(sx, sy), [x, y]| (sx + x, sy + y));
    (sum_x / points.len() as f64, sum_y / points.len() as f64)
}

/// Blank means "not recorded" (`None`) — dimensions are optional, unlike
/// size/price — so only a non-blank, non-numeric value is an error.
fn parse_optional_dimension(raw: &str) -> Result<Option<Decimal>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let value = Decimal::from_str(trimmed).map_err(|_| "Enter a valid dimension.".to_string())?;
    if value <= Decimal::ZERO {
        return Err("Dimensions must be greater than zero.".to_string());
    }
    Ok(Some(value))
}

#[component]
pub fn ProjectDetail() -> impl IntoView {
    let api = use_api();
    let currency = use_currency();
    let params = use_params_map();
    let auth = use_auth();
    let can_create_plot = has_permission(auth, PERM_PLOTS_CREATE);
    let can_edit_plot = has_permission(auth, PERM_PLOTS_EDIT);
    let can_bulk_import_plots = has_permission(auth, PERM_PLOTS_BULK_IMPORT);
    let can_reserve = has_permission(auth, PERM_PLOTS_TRANSACTIONS_CREATE);
    let can_create_quote = has_permission(auth, PERM_QUOTES_CREATE);

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
    let show_bulk_import = RwSignal::new(false);
    let show_map = RwSignal::new(false);

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
                                    <div style="display:flex; gap: var(--space-2);">
                                        {can_bulk_import_plots.then(|| view! {
                                            <button
                                                class="btn btn-secondary"
                                                on:click=move |_| {
                                                    show_bulk_import.update(|v| *v = !*v);
                                                    show_add_plot.set(false);
                                                }
                                            >
                                                {move || if show_bulk_import.get() { "Cancel" } else { "Bulk import" }}
                                            </button>
                                        })}
                                        {can_create_plot.then(|| view! {
                                            <button
                                                class="btn btn-secondary"
                                                on:click=move |_| {
                                                    show_add_plot.update(|v| *v = !*v);
                                                    show_bulk_import.set(false);
                                                }
                                            >
                                                {move || if show_add_plot.get() { "Cancel" } else { "+ Add plot" }}
                                            </button>
                                        })}
                                    </div>
                                </div>

                                <Show when=move || show_bulk_import.get()>
                                    <BulkPlotImport
                                        project_id=project_id_val
                                        on_imported=move || {
                                            plots.refetch();
                                        }
                                    />
                                </Show>

                                <Show when=move || show_add_plot.get()>
                                    <div class="card" style="margin-bottom: var(--space-4)">
                                        <AddPlotForm
                                            project_id=project_id_val
                                            project_code=p.code.clone()
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

        <div class="filter-tabs">
            <button
                type="button"
                class="filter-tab"
                class:active=move || !show_map.get()
                on:click=move |_| show_map.set(false)
            >
                "Grid"
            </button>
            <button
                type="button"
                class="filter-tab"
                class:active=move || show_map.get()
                on:click=move |_| show_map.set(true)
            >
                "Map"
            </button>
        </div>

        <Suspense fallback=|| view! { <LoadingState label="Loading plots…" /> }>
            {move || {
                plots
                    .get()
                    .map(|wrapped| wrapped.take())
                    .flatten()
                    .map(|result| match result {
                        // The map has to stay reachable even with zero
                        // plots yet — drawing a shape and clicking
                        // "Create Plot" on it (this phase's whole point)
                        // is now a legitimate way to get the *first*
                        // plot, so gating map access behind "at least
                        // one plot already exists" would be circular.
                        Ok(list) if show_map.get() => {
                            let Some(id) = project_id() else { return ().into_any() };
                            view! { <ProjectMapSection project_id=id plots=list selected=selected /> }
                                .into_any()
                        }
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
                                                    <span class="plot-dimensions">
                                                        {pwc.plot.size.to_string()} " acres"
                                                        {domain::format_dimensions(pwc.plot.side_1, pwc.plot.side_2, &pwc.plot.dimension_unit)
                                                            .map(|d| format!(" · {d}"))}
                                                    </span>
                                                    <span class="plot-price">{format_money(pwc.plot.asking_price, &currency.get())}</span>
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
                    let show_quote_form = RwSignal::new(false);
                    let show_edit_form = RwSignal::new(false);
                    let dimensions_text = domain::format_dimensions(
                        pwc.plot.side_1,
                        pwc.plot.side_2,
                        &pwc.plot.dimension_unit,
                    )
                        .unwrap_or_else(|| "Not specified".to_string());
                    let pwc_for_edit = pwc.clone();
                    view! {
                        <div class="card" style="margin-top: var(--space-4)">
                            <div class="page-header" style="margin-bottom: var(--space-3)">
                                <h2 class="mt-0">{pwc.plot.plot_number.clone()}</h2>
                                <StatusBadge label=pwc.status_label.clone() color=pwc.status_color.clone() />
                            </div>
                            <p>
                                "Size: " {pwc.plot.size.to_string()} " acres · Asking price: "
                                {format_money(pwc.plot.asking_price, &currency.get())}
                            </p>
                            <p>"Dimensions: " {dimensions_text}</p>
                            {pwc.plot.title_number.clone().map(|t| view! { <p>"Title: " {t}</p> })}

                            <PlotCommercialPosition project_id=pwc.plot.project_id plot_id=plot_id />

                            <DocumentsPanel entity_type=domain::DocumentEntityType::Plot entity_id=plot_id />

                            <TitleRecordsPanel plot_id=plot_id />

                            {can_edit_plot.then(|| view! {
                                <button
                                    type="button"
                                    class="btn btn-secondary"
                                    style="margin-bottom: var(--space-3);"
                                    on:click=move |_| show_edit_form.update(|v| *v = !*v)
                                >
                                    {move || if show_edit_form.get() { "Cancel edit" } else { "Edit plot" }}
                                </button>
                            })}

                            <Show when=move || show_edit_form.get()>
                                <EditPlotForm
                                    plot=pwc_for_edit.clone()
                                    on_saved=move |updated: PlotWithColor| {
                                        selected.set(Some(updated));
                                        show_edit_form.set(false);
                                        plots.refetch();
                                    }
                                />
                            </Show>

                            <Show when=move || startable && (can_reserve || can_create_quote)>
                                <Show when=move || can_reserve && can_create_quote>
                                    <div class="filter-tabs">
                                        <button
                                            type="button"
                                            class="filter-tab"
                                            class:active=move || !show_quote_form.get()
                                            on:click=move |_| show_quote_form.set(false)
                                        >
                                            "Reserve now"
                                        </button>
                                        <button
                                            type="button"
                                            class="filter-tab"
                                            class:active=move || show_quote_form.get()
                                            on:click=move |_| show_quote_form.set(true)
                                        >
                                            "Send a quotation"
                                        </button>
                                    </div>
                                </Show>

                                <Show
                                    when=move || if can_reserve && can_create_quote { show_quote_form.get() } else { can_create_quote }
                                    fallback=move || view! {
                                        <ReserveForm
                                            plot_id=plot_id
                                            project_id=pwc.plot.project_id
                                            asking_price=asking_price
                                            on_reserved=move || {
                                                plots.refetch();
                                                selected.set(None);
                                            }
                                        />
                                    }
                                >
                                    <QuoteForm
                                        plot_id=plot_id
                                        asking_price=asking_price
                                        on_quoted=move || {
                                            selected.set(None);
                                        }
                                    />
                                </Show>
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
fn AddPlotForm(
    project_id: Uuid,
    project_code: String,
    on_added: impl Fn() + Clone + 'static,
) -> impl IntoView {
    let api = use_api();
    let currency = use_currency();

    let plot_number = RwSignal::new(String::new());
    let size = RwSignal::new(String::new());
    let side_1 = RwSignal::new(String::new());
    let side_2 = RwSignal::new(String::new());
    let asking_price = RwSignal::new(String::new());
    let minimum_price = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);
    let submitting = RwSignal::new(false);
    let generating = RwSignal::new(false);

    let on_generate = {
        let api = api.clone();
        let project_code = project_code.clone();
        move |_| {
            if generating.get() {
                return;
            }
            generating.set(true);
            let api = api.clone();
            let project_code = project_code.clone();
            spawn_local(async move {
                match api.next_number("plot", Some(&project_code)).await {
                    Ok(number) => plot_number.set(number),
                    Err(e) => error.set(Some(format!("Couldn't generate a plot number: {e}"))),
                }
                generating.set(false);
            });
        }
    };

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
        let parsed_side_1 = match parse_optional_dimension(&side_1.get()) {
            Ok(v) => v,
            Err(msg) => {
                error.set(Some(msg));
                return;
            }
        };
        let parsed_side_2 = match parse_optional_dimension(&side_2.get()) {
            Ok(v) => v,
            Err(msg) => {
                error.set(Some(msg));
                return;
            }
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
            side_1: parsed_side_1,
            side_2: parsed_side_2,
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
                <div style="display:flex; gap: var(--space-2);">
                    <input
                        id="plot-number"
                        type="text"
                        required
                        prop:value=plot_number
                        on:input=move |ev| plot_number.set(event_target_value(&ev))
                    />
                    <button
                        type="button"
                        class="btn btn-secondary"
                        disabled=generating
                        on:click=on_generate
                    >
                        {move || if generating.get() { "…" } else { "Generate..." }}
                    </button>
                </div>
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
                <label>"Plot dimensions (feet)"</label>
                <div class="dimension-row">
                    <span class="dimension-label">"Side 1"</span>
                    <input
                        type="text"
                        inputmode="decimal"
                        placeholder="80"
                        aria-label="Side 1"
                        prop:value=side_1
                        on:input=move |ev| side_1.set(event_target_value(&ev))
                    />
                    <span class="dimension-label">"by"</span>
                    <span class="dimension-label">"Side 2"</span>
                    <input
                        type="text"
                        inputmode="decimal"
                        placeholder="100"
                        aria-label="Side 2"
                        prop:value=side_2
                        on:input=move |ev| side_2.set(event_target_value(&ev))
                    />
                    <span class="dimension-label">"ft"</span>
                </div>
            </div>

            <div class="field">
                <label for="asking-price">"Asking price (" {move || currency.get()} ")"</label>
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
                <label for="minimum-price">"Minimum price (" {move || currency.get()} ", optional)"</label>
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

/// Edits an existing plot's own fields — number, size, dimensions,
/// pricing. Deliberately doesn't touch status/ownership: those change
/// through the sales workflow (`ReserveForm`, `QuoteForm`, `approvals.rs`),
/// not here. Pre-fills from the plot currently selected in the grid/map,
/// and reports the freshly-saved record back via `on_saved` so the caller
/// can update `selected` without a second round trip.
#[component]
fn EditPlotForm(
    plot: PlotWithColor,
    on_saved: impl Fn(PlotWithColor) + Clone + 'static,
) -> impl IntoView {
    let api = use_api();
    let currency = use_currency();

    let project_id = plot.plot.project_id;
    let plot_id = plot.plot.id;
    let status_label = plot.status_label.clone();
    let status_color = plot.status_color.clone();

    let plot_number = RwSignal::new(plot.plot.plot_number.clone());
    let size = RwSignal::new(plot.plot.size.to_string());
    let side_1 = RwSignal::new(plot.plot.side_1.map(|v| v.to_string()).unwrap_or_default());
    let side_2 = RwSignal::new(plot.plot.side_2.map(|v| v.to_string()).unwrap_or_default());
    let asking_price = RwSignal::new(plot.plot.asking_price.to_string());
    let minimum_price = RwSignal::new(plot.plot.minimum_price.to_string());
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
        let parsed_side_1 = match parse_optional_dimension(&side_1.get()) {
            Ok(v) => v,
            Err(msg) => {
                error.set(Some(msg));
                return;
            }
        };
        let parsed_side_2 = match parse_optional_dimension(&side_2.get()) {
            Ok(v) => v,
            Err(msg) => {
                error.set(Some(msg));
                return;
            }
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
        let on_saved = on_saved.clone();
        let status_label = status_label.clone();
        let status_color = status_color.clone();
        let input = UpdatePlotInput {
            plot_number: plot_number.get(),
            size: parsed_size,
            side_1: parsed_side_1,
            side_2: parsed_side_2,
            asking_price: parsed_asking,
            minimum_price: parsed_minimum,
        };
        spawn_local(async move {
            match api.update_plot(project_id, plot_id, input).await {
                Ok(updated) => on_saved(PlotWithColor { plot: updated, status_label, status_color }),
                Err(e) => error.set(Some(format!("{e}"))),
            }
            submitting.set(false);
        });
    };

    view! {
        <form
            on:submit=on_submit
            style="margin-bottom: var(--space-4); padding: var(--space-3); border: 1px solid var(--color-border); border-radius: var(--radius-sm);"
        >
            <h3 class="mt-0">"Edit plot"</h3>
            {move || error.get().map(|msg| view! { <ErrorAlert message=msg /> })}

            <div class="field">
                <label for="edit-plot-number">"Plot number"</label>
                <input
                    id="edit-plot-number"
                    type="text"
                    required
                    prop:value=plot_number
                    on:input=move |ev| plot_number.set(event_target_value(&ev))
                />
            </div>

            <div class="field">
                <label for="edit-plot-size">"Size (acres)"</label>
                <input
                    id="edit-plot-size"
                    type="text"
                    inputmode="decimal"
                    required
                    prop:value=size
                    on:input=move |ev| size.set(event_target_value(&ev))
                />
            </div>

            <div class="field">
                <label>"Plot dimensions (feet)"</label>
                <div class="dimension-row">
                    <span class="dimension-label">"Side 1"</span>
                    <input
                        type="text"
                        inputmode="decimal"
                        placeholder="80"
                        aria-label="Side 1"
                        prop:value=side_1
                        on:input=move |ev| side_1.set(event_target_value(&ev))
                    />
                    <span class="dimension-label">"by"</span>
                    <span class="dimension-label">"Side 2"</span>
                    <input
                        type="text"
                        inputmode="decimal"
                        placeholder="100"
                        aria-label="Side 2"
                        prop:value=side_2
                        on:input=move |ev| side_2.set(event_target_value(&ev))
                    />
                    <span class="dimension-label">"ft"</span>
                </div>
            </div>

            <div class="field">
                <label for="edit-asking-price">"Asking price (" {move || currency.get()} ")"</label>
                <input
                    id="edit-asking-price"
                    type="text"
                    inputmode="numeric"
                    required
                    prop:value=asking_price
                    on:input=move |ev| asking_price.set(event_target_value(&ev))
                />
            </div>

            <div class="field">
                <label for="edit-minimum-price">"Minimum price (" {move || currency.get()} ", optional)"</label>
                <input
                    id="edit-minimum-price"
                    type="text"
                    inputmode="numeric"
                    prop:value=minimum_price
                    on:input=move |ev| minimum_price.set(event_target_value(&ev))
                />
            </div>

            <button type="submit" class="btn btn-primary" disabled=submitting>
                {move || if submitting.get() { "Saving…" } else { "Save changes" }}
            </button>
        </form>
    }
}

/// Uploads a CSV during tenant onboarding — parses client-side
/// (`crate::csv_import::parse_plots_csv`) so the user sees per-row
/// problems before anything reaches the server, then posts only the
/// rows that parsed cleanly to `POST /projects/:id/plots/bulk`, which
/// re-validates each one independently
/// (`crates/backend/src/routes/projects.rs::insert_plot`) — a row
/// that parses fine can still fail there (a duplicate plot number).
#[component]
fn BulkPlotImport(project_id: Uuid, on_imported: impl Fn() + Clone + Send + 'static) -> impl IntoView {
    let api = use_api();
    let rows: RwSignal<Vec<ParsedRow<CreatePlotInput>>> = RwSignal::new(Vec::new());
    let parsing = RwSignal::new(false);
    let importing = RwSignal::new(false);
    let import_result: RwSignal<Option<domain::BulkImportResult>> = RwSignal::new(None);
    let error = RwSignal::new(None::<String>);

    let on_file_change = move |ev: leptos::ev::Event| {
        let Some(input) = ev
            .target()
            .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
        else {
            return;
        };
        let Some(files) = input.files() else { return };
        let Some(file) = files.item(0) else { return };
        error.set(None);
        import_result.set(None);
        parsing.set(true);
        spawn_local(async move {
            let text = wasm_bindgen_futures::JsFuture::from(file.text())
                .await
                .ok()
                .and_then(|v| v.as_string());
            match text {
                Some(text) => rows.set(csv_import::parse_plots_csv(&text, project_id)),
                None => error.set(Some("Couldn't read that file.".to_string())),
            }
            parsing.set(false);
        });
    };

    let template_href = format!(
        "data:text/csv;charset=utf-8,{}",
        js_sys::encode_uri_component(csv_import::PLOTS_TEMPLATE)
    );

    view! {
        <div class="card" style="margin-bottom: var(--space-4)">
            <h3 class="mt-0">"Bulk import plots"</h3>
            <p class="meta mt-0">"CSV columns, in order: plot_number, size, side_1, side_2, asking_price, minimum_price. side_1/side_2 (feet) are optional — leave blank if not recorded."</p>
            <a
                href=template_href
                download="plots_template.csv"
                class="btn btn-secondary"
                style="margin-bottom: var(--space-3); display:inline-block;"
            >
                "Download CSV template"
            </a>

            {move || error.get().map(|msg| view! { <ErrorAlert message=msg /> })}

            <div class="field">
                <label for="bulk-plots-file">
                    {move || if parsing.get() { "Reading…" } else { "CSV file" }}
                </label>
                <input
                    id="bulk-plots-file"
                    type="file"
                    accept=".csv,text/csv"
                    disabled=parsing
                    on:change=on_file_change
                />
            </div>

            {move || {
                let parsed = rows.get();
                if parsed.is_empty() || import_result.get().is_some() {
                    return None;
                }
                let api = api.clone();
                let on_imported = on_imported.clone();
                let valid_count = parsed.iter().filter(|r| r.result.is_ok()).count();
                let error_count = parsed.len() - valid_count;
                Some(view! {
                    <p>
                        {format!(
                            "{} row(s) found — {} valid, {} with problems.",
                            parsed.len(), valid_count, error_count,
                        )}
                    </p>
                    {(error_count > 0).then(|| view! {
                        <ul class="meta">
                            {parsed
                                .iter()
                                .filter_map(|r| r.result.as_ref().err().map(|e| {
                                    view! { <li>"Row " {r.row} ": " {e.clone()}</li> }
                                }))
                                .collect_view()}
                        </ul>
                    })}
                    <button
                        class="btn btn-primary"
                        disabled=move || importing.get() || valid_count == 0
                        on:click=move |_| {
                            if importing.get() {
                                return;
                            }
                            error.set(None);
                            importing.set(true);
                            let api = api.clone();
                            let on_imported = on_imported.clone();
                            let valid: Vec<(u32, CreatePlotInput)> = rows
                                .get()
                                .into_iter()
                                .filter_map(|r| r.result.ok().map(|input| (r.row, input)))
                                .collect();
                            let original_rows: Vec<u32> = valid.iter().map(|(row, _)| *row).collect();
                            let inputs: Vec<CreatePlotInput> =
                                valid.into_iter().map(|(_, input)| input).collect();
                            spawn_local(async move {
                                match api.bulk_create_plots(project_id, inputs).await {
                                    Ok(mut r) => {
                                        for e in r.errors.iter_mut() {
                                            if let Some(&orig) = original_rows.get(e.row as usize - 1) {
                                                e.row = orig;
                                            }
                                        }
                                        let had_created = r.created > 0;
                                        import_result.set(Some(r));
                                        if had_created {
                                            on_imported();
                                        }
                                    }
                                    Err(e) => error.set(Some(format!("{e}"))),
                                }
                                importing.set(false);
                            });
                        }
                    >
                        {move || {
                            if importing.get() {
                                "Importing…".to_string()
                            } else {
                                format!("Import {valid_count} plot(s)")
                            }
                        }}
                    </button>
                })
            }}

            {move || import_result.get().map(|r| view! {
                <div class="alert alert-warning">
                    <p>{format!("{} plot(s) imported.", r.created)}</p>
                    {(!r.errors.is_empty()).then(|| view! {
                        <ul>
                            {r.errors
                                .iter()
                                .map(|e| view! { <li>"Row " {e.row} ": " {e.message.clone()}</li> })
                                .collect_view()}
                        </ul>
                    })}
                </div>
            })}
        </div>
    }
}

/// The plot's full commercial position — reservation/sale, buyer,
/// purchase type, and (for Lipa Pole Pole) the linked loan account's
/// payment/finance position, all in one place so a director doesn't
/// have to cross-reference the Finance module to answer "where does
/// this plot actually stand?" Plot Status (the badge above this, in
/// the caller) and Finance Status here are deliberately two separate
/// fields, never merged into one — a plot can be Sold while its
/// account is still merely Performing, and collapsing that into a
/// single status would hide exactly the distinction this exists to
/// show.
#[component]
fn PlotCommercialPosition(project_id: Uuid, plot_id: Uuid) -> impl IntoView {
    let api = use_api();
    let currency = use_currency();

    let summary = LocalResource::new(move || {
        let api = api.clone();
        async move { api.get_plot_commercial_summary(project_id, plot_id).await }
    });

    view! {
        <div class="commercial-position">
            <Suspense fallback=|| view! { <LoadingState label="Loading commercial position…" /> }>
                {move || {
                    let currency = currency.get();
                    summary
                        .get()
                        .map(|wrapped| wrapped.take())
                        .map(move |result| match result {
                            Ok(s) => match s.sale {
                                None => view! {
                                    <p class="meta">"Available — No active reservation or buyer."</p>
                                }.into_any(),
                                Some(sale) => {
                                    let purchase_type = format_payment_mode(sale.payment_mode);
                                    view! {
                                        <div class="form-grid-2 commercial-position-grid">
                                            <div>
                                                <span class="meta">"Customer / Buyer"</span>
                                                <p class="mt-0">{sale.customer_name.clone()}</p>
                                            </div>
                                            <div>
                                                <span class="meta">"Purchase Type"</span>
                                                <p class="mt-0">{purchase_type}</p>
                                            </div>
                                            <div>
                                                <span class="meta">"Selling Price"</span>
                                                <p class="mt-0">{format_money(sale.agreed_price, &currency)}</p>
                                            </div>
                                            {(!sale.co_buyers.is_empty()).then(|| {
                                                let names: Vec<String> = sale.co_buyers.iter().map(|c| c.customer_name.clone()).collect();
                                                view! {
                                                    <div>
                                                        <span class="meta">"Co-buyers"</span>
                                                        <p class="mt-0">{names.join(", ")}</p>
                                                    </div>
                                                }
                                            })}
                                            {(!sale.additional_plots.is_empty()).then(|| {
                                                let numbers: Vec<String> = sale.additional_plots.iter().map(|p| p.plot_number.clone()).collect();
                                                view! {
                                                    <div>
                                                        <span class="meta">"Also includes plots"</span>
                                                        <p class="mt-0">{numbers.join(", ")}</p>
                                                    </div>
                                                }
                                            })}
                                            {match &sale.loan_account {
                                                None => view! {
                                                    <div>
                                                        <span class="meta">"Payment Status"</span>
                                                        <p class="mt-0">"Paid in full (cash)"</p>
                                                    </div>
                                                }.into_any(),
                                                Some(loan) => {
                                                    let label = sale.loan_status_label.clone().unwrap_or_default();
                                                    let color = sale.loan_status_color.clone().unwrap_or_else(|| "#6b7280".to_string());
                                                    let interest_text = match loan.interest_rate {
                                                        Some(rate) => format!("{rate}% p.a."),
                                                        None => "No interest".to_string(),
                                                    };
                                                    view! {
                                                        <div>
                                                            <span class="meta">"Deposit"</span>
                                                            <p class="mt-0">
                                                                {format_money(loan.deposit_paid, &currency)} " of "
                                                                {format_money(loan.deposit_required, &currency)}
                                                            </p>
                                                        </div>
                                                        <div>
                                                            <span class="meta">"Total Paid"</span>
                                                            <p class="mt-0">{format_money(loan.amount_paid, &currency)}</p>
                                                        </div>
                                                        <div>
                                                            <span class="meta">"Outstanding Balance"</span>
                                                            <p class="mt-0">{format_money(loan.outstanding_balance, &currency)}</p>
                                                        </div>
                                                        <div>
                                                            <span class="meta">"Finance Status"</span>
                                                            <p class="mt-0"><StatusBadge label=label color=color /></p>
                                                        </div>
                                                        <div>
                                                            <span class="meta">"Interest"</span>
                                                            <p class="mt-0">{interest_text}</p>
                                                        </div>
                                                        {loan.next_instalment_due_date.map(|due| {
                                                            let amount = loan.next_instalment_amount.unwrap_or_default();
                                                            view! {
                                                                <div>
                                                                    <span class="meta">"Next Instalment"</span>
                                                                    <p class="mt-0">{format_money(amount, &currency)} " due " {due.to_string()}</p>
                                                                </div>
                                                            }
                                                        })}
                                                        {(loan.days_in_arrears > 0).then(|| view! {
                                                            <div>
                                                                <span class="meta">"Days Overdue"</span>
                                                                <p class="mt-0" style="color: var(--color-danger); font-weight: 700;">
                                                                    {loan.days_in_arrears}
                                                                </p>
                                                            </div>
                                                        })}
                                                    }.into_any()
                                                }
                                            }}
                                        </div>
                                    }.into_any()
                                }
                            },
                            Err(e) => view! { <ErrorAlert message=format!("Couldn't load this plot's commercial position: {e}") /> }.into_any(),
                        })
                }}
            </Suspense>
        </div>
    }
}

fn title_status_value(s: domain::TitleStatus) -> &'static str {
    match s {
        domain::TitleStatus::MotherTitle => "mother_title",
        domain::TitleStatus::IndividualTitle => "individual_title",
        domain::TitleStatus::PendingRegistration => "pending_registration",
        domain::TitleStatus::Disputed => "disputed",
        domain::TitleStatus::Cancelled => "cancelled",
    }
}

fn title_status_from_value(v: &str) -> domain::TitleStatus {
    match v {
        "mother_title" => domain::TitleStatus::MotherTitle,
        "pending_registration" => domain::TitleStatus::PendingRegistration,
        "disputed" => domain::TitleStatus::Disputed,
        "cancelled" => domain::TitleStatus::Cancelled,
        _ => domain::TitleStatus::IndividualTitle,
    }
}

fn title_status_label(s: domain::TitleStatus) -> &'static str {
    match s {
        domain::TitleStatus::MotherTitle => "Mother Title",
        domain::TitleStatus::IndividualTitle => "Individual Title",
        domain::TitleStatus::PendingRegistration => "Pending Registration",
        domain::TitleStatus::Disputed => "Disputed",
        domain::TitleStatus::Cancelled => "Cancelled",
    }
}

fn transfer_status_value(s: domain::TransferStatus) -> &'static str {
    match s {
        domain::TransferStatus::NotStarted => "not_started",
        domain::TransferStatus::InProgress => "in_progress",
        domain::TransferStatus::Completed => "completed",
    }
}

fn transfer_status_from_value(v: &str) -> domain::TransferStatus {
    match v {
        "in_progress" => domain::TransferStatus::InProgress,
        "completed" => domain::TransferStatus::Completed,
        _ => domain::TransferStatus::NotStarted,
    }
}

fn transfer_status_label(s: domain::TransferStatus) -> &'static str {
    match s {
        domain::TransferStatus::NotStarted => "Not Started",
        domain::TransferStatus::InProgress => "In Progress",
        domain::TransferStatus::Completed => "Completed",
    }
}

fn parse_date_field(s: &str) -> Option<chrono::NaiveDate> {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

/// A plot's title/ownership history (`domain::TitleRecord` — see its
/// module docs). Separate from `Plot::title_number` (the plain string
/// shown just above this card): that field is "the title on record
/// right now", this is the structured history behind it — a mother
/// title becoming an individual title, a transfer in progress, a chain
/// of registered owners. Each record can carry its own attached
/// documents (a title deed, a survey plan) via the generic
/// `DocumentsPanel`, toggled open per row rather than always rendered
/// — most rows are old history nobody needs to re-open.
#[component]
fn TitleRecordsPanel(plot_id: Uuid) -> impl IntoView {
    let api = use_api();
    let auth = use_auth();
    let can_manage = has_permission(auth, domain::PERM_TITLES_MANAGE);

    let refresh = RwSignal::new(0u32);
    let add_open = RwSignal::new(false);
    let add_error = RwSignal::new(None::<String>);
    let add_saving = RwSignal::new(false);
    let editing_id = RwSignal::new(None::<Uuid>);
    let edit_error = RwSignal::new(None::<String>);
    let edit_saving = RwSignal::new(false);
    let docs_open_id = RwSignal::new(None::<Uuid>);

    let a_title_number = RwSignal::new(String::new());
    let a_owner = RwSignal::new(String::new());
    let a_previous_owner = RwSignal::new(String::new());
    let a_title_status = RwSignal::new(title_status_value(domain::TitleStatus::IndividualTitle).to_string());
    let a_transfer_status = RwSignal::new(transfer_status_value(domain::TransferStatus::NotStarted).to_string());
    let a_issue_date = RwSignal::new(String::new());
    let a_registration_date = RwSignal::new(String::new());
    let a_transfer_date = RwSignal::new(String::new());
    let a_notes = RwSignal::new(String::new());

    let e_title_number = RwSignal::new(String::new());
    let e_owner = RwSignal::new(String::new());
    let e_previous_owner = RwSignal::new(String::new());
    let e_title_status = RwSignal::new(String::new());
    let e_transfer_status = RwSignal::new(String::new());
    let e_issue_date = RwSignal::new(String::new());
    let e_registration_date = RwSignal::new(String::new());
    let e_transfer_date = RwSignal::new(String::new());
    let e_notes = RwSignal::new(String::new());

    let records = LocalResource::new({
        let api = api.clone();
        move || {
            let api = api.clone();
            refresh.get();
            async move { api.list_title_records(plot_id).await }
        }
    });

    let api_for_add = api.clone();
    let on_add = move |_| {
        if add_saving.get() {
            return;
        }
        add_error.set(None);
        add_saving.set(true);
        let api = api_for_add.clone();
        let input = domain::CreateTitleRecordInput {
            title_number: a_title_number.get(),
            registered_owner_name: a_owner.get(),
            previous_owner_name: Some(a_previous_owner.get()).filter(|s| !s.trim().is_empty()),
            title_status: title_status_from_value(&a_title_status.get()),
            transfer_status: transfer_status_from_value(&a_transfer_status.get()),
            issue_date: parse_date_field(&a_issue_date.get()),
            registration_date: parse_date_field(&a_registration_date.get()),
            transfer_date: parse_date_field(&a_transfer_date.get()),
            notes: Some(a_notes.get()).filter(|s| !s.trim().is_empty()),
        };
        spawn_local(async move {
            match api.create_title_record(plot_id, input).await {
                Ok(_) => {
                    a_title_number.set(String::new());
                    a_owner.set(String::new());
                    a_previous_owner.set(String::new());
                    a_title_status.set(title_status_value(domain::TitleStatus::IndividualTitle).to_string());
                    a_transfer_status.set(transfer_status_value(domain::TransferStatus::NotStarted).to_string());
                    a_issue_date.set(String::new());
                    a_registration_date.set(String::new());
                    a_transfer_date.set(String::new());
                    a_notes.set(String::new());
                    add_open.set(false);
                    refresh.update(|n| *n += 1);
                }
                Err(e) => add_error.set(Some(format!("{e}"))),
            }
            add_saving.set(false);
        });
    };

    let api_for_edit = api.clone();
    let on_save_edit = move |id: Uuid| {
        if edit_saving.get() {
            return;
        }
        edit_error.set(None);
        edit_saving.set(true);
        let api = api_for_edit.clone();
        let input = domain::UpdateTitleRecordInput {
            title_number: e_title_number.get(),
            registered_owner_name: e_owner.get(),
            previous_owner_name: Some(e_previous_owner.get()).filter(|s| !s.trim().is_empty()),
            title_status: title_status_from_value(&e_title_status.get()),
            transfer_status: transfer_status_from_value(&e_transfer_status.get()),
            issue_date: parse_date_field(&e_issue_date.get()),
            registration_date: parse_date_field(&e_registration_date.get()),
            transfer_date: parse_date_field(&e_transfer_date.get()),
            notes: Some(e_notes.get()).filter(|s| !s.trim().is_empty()),
        };
        spawn_local(async move {
            match api.update_title_record(id, input).await {
                Ok(_) => {
                    editing_id.set(None);
                    refresh.update(|n| *n += 1);
                }
                Err(e) => edit_error.set(Some(format!("{e}"))),
            }
            edit_saving.set(false);
        });
    };

    view! {
        <div class="card form-card" style="margin-bottom: var(--space-5)">
            <div class="page-header" style="margin-bottom: var(--space-3)">
                <h2 class="mt-0">"Title & Ownership"</h2>
                {can_manage.then(|| {
                    view! {
                        <button
                            type="button"
                            class="btn btn-secondary"
                            on:click=move |_| add_open.update(|v| *v = !*v)
                        >
                            {move || if add_open.get() { "Cancel" } else { "Add title record" }}
                        </button>
                    }
                })}
            </div>

            {move || if add_open.get() {
                let on_add = on_add.clone();
                view! {
                    <div class="form-grid-2" style="margin-bottom: var(--space-3)">
                        <div class="field">
                            <label for="tr-add-number">"Title number"</label>
                            <input id="tr-add-number" type="text" required prop:value=a_title_number on:input=move |ev| a_title_number.set(event_target_value(&ev)) />
                        </div>
                        <div class="field">
                            <label for="tr-add-owner">"Registered owner"</label>
                            <input id="tr-add-owner" type="text" required prop:value=a_owner on:input=move |ev| a_owner.set(event_target_value(&ev)) />
                        </div>
                        <div class="field">
                            <label for="tr-add-prev-owner">"Previous owner"</label>
                            <input id="tr-add-prev-owner" type="text" prop:value=a_previous_owner on:input=move |ev| a_previous_owner.set(event_target_value(&ev)) />
                        </div>
                        <div class="field">
                            <label for="tr-add-title-status">"Title status"</label>
                            <select id="tr-add-title-status" prop:value=a_title_status on:change=move |ev| a_title_status.set(event_target_value(&ev))>
                                <option value="mother_title">"Mother Title"</option>
                                <option value="individual_title">"Individual Title"</option>
                                <option value="pending_registration">"Pending Registration"</option>
                                <option value="disputed">"Disputed"</option>
                                <option value="cancelled">"Cancelled"</option>
                            </select>
                        </div>
                        <div class="field">
                            <label for="tr-add-transfer-status">"Transfer status"</label>
                            <select id="tr-add-transfer-status" prop:value=a_transfer_status on:change=move |ev| a_transfer_status.set(event_target_value(&ev))>
                                <option value="not_started">"Not Started"</option>
                                <option value="in_progress">"In Progress"</option>
                                <option value="completed">"Completed"</option>
                            </select>
                        </div>
                        <div class="field">
                            <label for="tr-add-issue-date">"Issue date"</label>
                            <input id="tr-add-issue-date" type="date" prop:value=a_issue_date on:input=move |ev| a_issue_date.set(event_target_value(&ev)) />
                        </div>
                        <div class="field">
                            <label for="tr-add-reg-date">"Registration date"</label>
                            <input id="tr-add-reg-date" type="date" prop:value=a_registration_date on:input=move |ev| a_registration_date.set(event_target_value(&ev)) />
                        </div>
                        <div class="field">
                            <label for="tr-add-transfer-date">"Transfer date"</label>
                            <input id="tr-add-transfer-date" type="date" prop:value=a_transfer_date on:input=move |ev| a_transfer_date.set(event_target_value(&ev)) />
                        </div>
                        <div class="field" style="grid-column: 1 / -1">
                            <label for="tr-add-notes">"Notes"</label>
                            <input id="tr-add-notes" type="text" prop:value=a_notes on:input=move |ev| a_notes.set(event_target_value(&ev)) />
                        </div>
                    </div>
                    {move || add_error.get().map(|msg| view! { <ErrorAlert message=msg /> })}
                    <button
                        type="button"
                        class="btn btn-primary"
                        style="margin-bottom: var(--space-3)"
                        disabled=move || add_saving.get() || a_title_number.get().trim().is_empty() || a_owner.get().trim().is_empty()
                        on:click=on_add
                    >
                        {move || if add_saving.get() { "Saving…" } else { "Save title record" }}
                    </button>
                }.into_any()
            } else {
                view! {}.into_any()
            }}

            <Suspense fallback=|| view! { <LoadingState label="Loading title history…" /> }>
                {move || {
                    records.get()
                        .map(|wrapped| wrapped.take())
                        .map(|result| match result {
                            Ok(list) if list.is_empty() => view! {
                                <p class="meta">"No title records yet."</p>
                            }.into_any(),
                            Ok(list) => {
                                view! {
                                    <div class="table-scroll">
                                    <table class="data-table">
                                        <thead>
                                            <tr>
                                                <th>"Title number"</th>
                                                <th>"Registered owner"</th>
                                                <th>"Title status"</th>
                                                <th>"Transfer status"</th>
                                                <th>"Recorded"</th>
                                                <th></th>
                                            </tr>
                                        </thead>
                                        <tbody>
                                            {list.into_iter().map(|rec| {
                                                let rec_id = rec.id;
                                                let is_editing = move || editing_id.get() == Some(rec_id);
                                                let is_docs_open = move || docs_open_id.get() == Some(rec_id);
                                                let rec_for_edit = rec.clone();
                                                let on_save_edit = on_save_edit.clone();
                                                view! {
                                                    <tr>
                                                        <td>{rec.title_number.clone()}</td>
                                                        <td>
                                                            {rec.registered_owner_name.clone()}
                                                            {rec.previous_owner_name.clone().map(|p| view! {
                                                                <div class="meta">"from " {p}</div>
                                                            })}
                                                        </td>
                                                        <td>{title_status_label(rec.title_status)}</td>
                                                        <td>{transfer_status_label(rec.transfer_status)}</td>
                                                        <td><span class="meta">{rec.created_at.format("%d %b %Y").to_string()} " · " {rec.created_by_name.clone()}</span></td>
                                                        <td>
                                                            <div style="display: flex; gap: var(--space-2)">
                                                                {can_manage.then(|| {
                                                                    let rec_for_edit = rec_for_edit.clone();
                                                                    view! {
                                                                        <button
                                                                            type="button"
                                                                            class="btn btn-secondary btn-sm"
                                                                            on:click=move |_| {
                                                                                if is_editing() {
                                                                                    editing_id.set(None);
                                                                                } else {
                                                                                    e_title_number.set(rec_for_edit.title_number.clone());
                                                                                    e_owner.set(rec_for_edit.registered_owner_name.clone());
                                                                                    e_previous_owner.set(rec_for_edit.previous_owner_name.clone().unwrap_or_default());
                                                                                    e_title_status.set(title_status_value(rec_for_edit.title_status).to_string());
                                                                                    e_transfer_status.set(transfer_status_value(rec_for_edit.transfer_status).to_string());
                                                                                    e_issue_date.set(rec_for_edit.issue_date.map(|d| d.to_string()).unwrap_or_default());
                                                                                    e_registration_date.set(rec_for_edit.registration_date.map(|d| d.to_string()).unwrap_or_default());
                                                                                    e_transfer_date.set(rec_for_edit.transfer_date.map(|d| d.to_string()).unwrap_or_default());
                                                                                    e_notes.set(rec_for_edit.notes.clone().unwrap_or_default());
                                                                                    edit_error.set(None);
                                                                                    editing_id.set(Some(rec_id));
                                                                                }
                                                                            }
                                                                        >
                                                                            {move || if is_editing() { "Cancel" } else { "Edit" }}
                                                                        </button>
                                                                    }
                                                                })}
                                                                <button
                                                                    type="button"
                                                                    class="btn btn-secondary btn-sm"
                                                                    on:click=move |_| {
                                                                        docs_open_id.update(|v| {
                                                                            *v = if *v == Some(rec_id) { None } else { Some(rec_id) };
                                                                        });
                                                                    }
                                                                >
                                                                    {move || if is_docs_open() { "Hide documents" } else { "Documents" }}
                                                                </button>
                                                            </div>
                                                        </td>
                                                    </tr>
                                                    {move || if is_editing() {
                                                        let on_save_edit = on_save_edit.clone();
                                                        view! {
                                                            <tr>
                                                                <td colspan="6">
                                                                    <div class="form-grid-2" style="margin: var(--space-2) 0">
                                                                        <div class="field">
                                                                            <label for="tr-edit-number">"Title number"</label>
                                                                            <input id="tr-edit-number" type="text" required prop:value=e_title_number on:input=move |ev| e_title_number.set(event_target_value(&ev)) />
                                                                        </div>
                                                                        <div class="field">
                                                                            <label for="tr-edit-owner">"Registered owner"</label>
                                                                            <input id="tr-edit-owner" type="text" required prop:value=e_owner on:input=move |ev| e_owner.set(event_target_value(&ev)) />
                                                                        </div>
                                                                        <div class="field">
                                                                            <label for="tr-edit-prev-owner">"Previous owner"</label>
                                                                            <input id="tr-edit-prev-owner" type="text" prop:value=e_previous_owner on:input=move |ev| e_previous_owner.set(event_target_value(&ev)) />
                                                                        </div>
                                                                        <div class="field">
                                                                            <label for="tr-edit-title-status">"Title status"</label>
                                                                            <select id="tr-edit-title-status" prop:value=e_title_status on:change=move |ev| e_title_status.set(event_target_value(&ev))>
                                                                                <option value="mother_title">"Mother Title"</option>
                                                                                <option value="individual_title">"Individual Title"</option>
                                                                                <option value="pending_registration">"Pending Registration"</option>
                                                                                <option value="disputed">"Disputed"</option>
                                                                                <option value="cancelled">"Cancelled"</option>
                                                                            </select>
                                                                        </div>
                                                                        <div class="field">
                                                                            <label for="tr-edit-transfer-status">"Transfer status"</label>
                                                                            <select id="tr-edit-transfer-status" prop:value=e_transfer_status on:change=move |ev| e_transfer_status.set(event_target_value(&ev))>
                                                                                <option value="not_started">"Not Started"</option>
                                                                                <option value="in_progress">"In Progress"</option>
                                                                                <option value="completed">"Completed"</option>
                                                                            </select>
                                                                        </div>
                                                                        <div class="field">
                                                                            <label for="tr-edit-issue-date">"Issue date"</label>
                                                                            <input id="tr-edit-issue-date" type="date" prop:value=e_issue_date on:input=move |ev| e_issue_date.set(event_target_value(&ev)) />
                                                                        </div>
                                                                        <div class="field">
                                                                            <label for="tr-edit-reg-date">"Registration date"</label>
                                                                            <input id="tr-edit-reg-date" type="date" prop:value=e_registration_date on:input=move |ev| e_registration_date.set(event_target_value(&ev)) />
                                                                        </div>
                                                                        <div class="field">
                                                                            <label for="tr-edit-transfer-date">"Transfer date"</label>
                                                                            <input id="tr-edit-transfer-date" type="date" prop:value=e_transfer_date on:input=move |ev| e_transfer_date.set(event_target_value(&ev)) />
                                                                        </div>
                                                                        <div class="field" style="grid-column: 1 / -1">
                                                                            <label for="tr-edit-notes">"Notes"</label>
                                                                            <input id="tr-edit-notes" type="text" prop:value=e_notes on:input=move |ev| e_notes.set(event_target_value(&ev)) />
                                                                        </div>
                                                                    </div>
                                                                    {move || edit_error.get().map(|msg| view! { <ErrorAlert message=msg /> })}
                                                                    <button
                                                                        type="button"
                                                                        class="btn btn-primary btn-sm"
                                                                        disabled=move || edit_saving.get()
                                                                        on:click=move |_| on_save_edit(rec_id)
                                                                    >
                                                                        {move || if edit_saving.get() { "Saving…" } else { "Save changes" }}
                                                                    </button>
                                                                </td>
                                                            </tr>
                                                        }.into_any()
                                                    } else {
                                                        view! {}.into_any()
                                                    }}
                                                    {move || if is_docs_open() {
                                                        view! {
                                                            <tr>
                                                                <td colspan="6">
                                                                    <DocumentsPanel entity_type=domain::DocumentEntityType::TitleRecord entity_id=rec_id />
                                                                </td>
                                                            </tr>
                                                        }.into_any()
                                                    } else {
                                                        view! {}.into_any()
                                                    }}
                                                }
                                            }).collect_view()}
                                        </tbody>
                                    </table>
                                    </div>
                                }.into_any()
                            }
                            Err(e) => view! { <ErrorAlert message=format!("Couldn't load title history: {e}") /> }.into_any(),
                        })
                }}
            </Suspense>
        </div>
    }
}

/// Starts a sale for the selected plot: pick a customer and payment mode,
/// confirm the price. This is the first step of the sales workflow
/// (docs/07) — deposit/tenor/schedule setup for Lipa Pole Pole comes
/// later (docs/08), not part of this form.
#[component]
fn ReserveForm(
    plot_id: Uuid,
    project_id: Uuid,
    asking_price: Decimal,
    on_reserved: impl Fn() + Clone + 'static,
) -> impl IntoView {
    let api = use_api();
    let currency = use_currency();

    // For "Additional plots" — other plots in this same project one
    // loan/agreement can also cover (legacy data migration readiness:
    // e.g. one loan across PL.7,8,9,10). Only plots that could
    // actually start a sale are offered.
    let project_plots = LocalResource::new({
        let api = api.clone();
        move || {
            let api = api.clone();
            async move { api.list_plots(project_id).await }
        }
    });
    let additional_plot_ids = RwSignal::new(Vec::<Uuid>::new());
    let show_additional_plots = RwSignal::new(false);

    // For "Additional buyers" — real joint buyers (legacy data
    // migration readiness: a customer register row like "CATHERINE A
    // OHOLA/ELIZABETH A OHOLA" recorded as two actual customers, not
    // one name concatenated together).
    let additional_customer_ids = RwSignal::new(Vec::<Uuid>::new());
    let show_additional_customers = RwSignal::new(false);

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

        let additional_customers = additional_customer_ids
            .get()
            .into_iter()
            .map(|customer_id| domain::AdditionalSaleCustomer { customer_id, role: domain::SaleCustomerRole::Joint })
            .collect();

        submitting.set(true);
        let api = api.clone();
        let on_reserved = on_reserved.clone();
        spawn_local(async move {
            let result = api
                .create_sale(CreateSaleInput {
                    plot_id,
                    customer_id: customer,
                    payment_mode: mode,
                    additional_plot_ids: additional_plot_ids.get_untracked(),
                    additional_customers,
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
                <label for="price">"Agreed price (" {move || currency.get()} ")"</label>
                <input
                    id="price"
                    type="text"
                    inputmode="numeric"
                    required
                    prop:value=price
                    on:input=move |ev| price.set(event_target_value(&ev))
                />
            </div>

            <button
                type="button"
                class="btn btn-secondary"
                style="margin-bottom: var(--space-2);"
                on:click=move |_| show_additional_plots.update(|v| *v = !*v)
            >
                {move || if show_additional_plots.get() { "Hide additional plots" } else { "+ Additional plots (one loan across several plots)" }}
            </button>
            <Show when=move || show_additional_plots.get()>
                <div class="field">
                    <Suspense fallback=|| ()>
                        {move || {
                            project_plots.get().map(|wrapped| wrapped.take()).map(|result| match result {
                                Ok(list) => list
                                    .into_iter()
                                    .filter(|pwc| pwc.plot.id != plot_id && matches!(pwc.plot.status, PlotStatus::Available))
                                    .map(|pwc| {
                                        let pid = pwc.plot.id;
                                        view! {
                                            <label class="checkbox-field">
                                                <input
                                                    type="checkbox"
                                                    on:change=move |ev| {
                                                        let checked = event_target_checked(&ev);
                                                        additional_plot_ids.update(|list| {
                                                            if checked {
                                                                if !list.contains(&pid) { list.push(pid); }
                                                            } else {
                                                                list.retain(|id| *id != pid);
                                                            }
                                                        });
                                                    }
                                                />
                                                {pwc.plot.plot_number.clone()}
                                            </label>
                                        }
                                    })
                                    .collect_view()
                                    .into_any(),
                                Err(_) => ().into_any(),
                            })
                        }}
                    </Suspense>
                </div>
            </Show>

            <button
                type="button"
                class="btn btn-secondary"
                style="margin-bottom: var(--space-2);"
                on:click=move |_| show_additional_customers.update(|v| *v = !*v)
            >
                {move || if show_additional_customers.get() { "Hide additional buyers" } else { "+ Additional buyers (joint purchase)" }}
            </button>
            <Show when=move || show_additional_customers.get()>
                <div class="field">
                    <Suspense fallback=|| ()>
                        {move || {
                            customers.get().map(|wrapped| wrapped.take()).map(|result| match result {
                                Ok(list) => list
                                    .into_iter()
                                    .filter(|c| customer_id.get() != c.customer.id.to_string())
                                    .map(|c| {
                                        let cid = c.customer.id;
                                        view! {
                                            <label class="checkbox-field">
                                                <input
                                                    type="checkbox"
                                                    on:change=move |ev| {
                                                        let checked = event_target_checked(&ev);
                                                        additional_customer_ids.update(|list| {
                                                            if checked {
                                                                if !list.contains(&cid) { list.push(cid); }
                                                            } else {
                                                                list.retain(|id| *id != cid);
                                                            }
                                                        });
                                                    }
                                                />
                                                {c.customer.full_name.clone()}
                                            </label>
                                        }
                                    })
                                    .collect_view()
                                    .into_any(),
                                Err(_) => ().into_any(),
                            })
                        }}
                    </Suspense>
                </div>
            </Show>

            <button type="submit" class="btn btn-primary" disabled=submitting>
                {move || if submitting.get() { "Reserving…" } else { "Reserve for customer" }}
            </button>
        </form>
    }
}

/// Creates a `Quotation` in Draft status instead of committing to a sale
/// directly — see docs/07's "quotations and offer letters" funnel stage
/// and `domain::Quotation`'s module docs for why this exists as a
/// separate step from `ReserveForm`. The quotation still needs to be
/// sent and accepted (from /quotations) before it becomes a real sale.
#[component]
fn QuoteForm(plot_id: Uuid, asking_price: Decimal, on_quoted: impl Fn() + Clone + 'static) -> impl IntoView {
    let api = use_api();
    let currency = use_currency();

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
    let valid_until = RwSignal::new(
        (chrono::Utc::now().date_naive() + chrono::Duration::days(7)).to_string(),
    );
    let notes = RwSignal::new(String::new());
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
        let Ok(quoted_price) = Decimal::from_str(price.get().trim()) else {
            error.set(Some("Enter a valid price.".to_string()));
            return;
        };
        let Ok(valid_until_date) = chrono::NaiveDate::parse_from_str(&valid_until.get(), "%Y-%m-%d") else {
            error.set(Some("Choose a validity date.".to_string()));
            return;
        };
        let mode = match payment_mode.get().as_str() {
            "lpp_free" => PaymentMode::LipaPolePoleInterestFree,
            "lpp_bearing" => PaymentMode::LipaPolePoleInterestBearing,
            _ => PaymentMode::FullCash,
        };

        submitting.set(true);
        let api = api.clone();
        let on_quoted = on_quoted.clone();
        spawn_local(async move {
            let result = api
                .create_quotation(CreateQuotationInput {
                    plot_id,
                    customer_id: customer,
                    payment_mode: mode,
                    quoted_price,
                    valid_until: valid_until_date,
                    notes: Some(notes.get()).filter(|s| !s.trim().is_empty()),
                })
                .await;
            match result {
                Ok(_) => on_quoted(),
                Err(e) => {
                    error.set(Some(format!("{e}")));
                    submitting.set(false);
                }
            }
        });
    };

    view! {
        <form on:submit=on_submit style="margin: var(--space-4) 0; padding-top: var(--space-4); border-top: 1px solid var(--color-border);">
            <h3>"Send a quotation"</h3>
            <p class="meta">"The customer can accept it later from Quotations — nothing changes for this plot until then."</p>
            {move || error.get().map(|msg| view! { <ErrorAlert message=msg /> })}

            <div class="field">
                <label for="quote-customer">"Customer"</label>
                <select
                    id="quote-customer"
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
            </div>

            <div class="field">
                <label for="quote-payment-mode">"Payment mode"</label>
                <select
                    id="quote-payment-mode"
                    prop:value=payment_mode
                    on:change=move |ev| payment_mode.set(event_target_value(&ev))
                >
                    <option value="full_cash">"Full cash"</option>
                    <option value="lpp_free">"Lipa Pole Pole (interest-free)"</option>
                    <option value="lpp_bearing">"Lipa Pole Pole (interest-bearing)"</option>
                </select>
            </div>

            <div class="field">
                <label for="quote-price">"Quoted price (" {move || currency.get()} ")"</label>
                <input
                    id="quote-price"
                    type="text"
                    inputmode="numeric"
                    required
                    prop:value=price
                    on:input=move |ev| price.set(event_target_value(&ev))
                />
            </div>

            <div class="field">
                <label for="quote-valid-until">"Valid until"</label>
                <input
                    id="quote-valid-until"
                    type="date"
                    required
                    prop:value=valid_until
                    on:input=move |ev| valid_until.set(event_target_value(&ev))
                />
            </div>

            <div class="field">
                <label for="quote-notes">"Notes"</label>
                <textarea
                    id="quote-notes"
                    prop:value=notes
                    on:input=move |ev| notes.set(event_target_value(&ev))
                ></textarea>
            </div>

            <button type="submit" class="btn btn-primary" disabled=submitting>
                {move || if submitting.get() { "Creating…" } else { "Create quotation" }}
            </button>
        </form>
    }
}

/// Minimal interactive-map v1 (see `domain::ProjectMap`'s module docs)
/// — loads the current map (if any) and hands off to `MapUploadForm`
/// or `MapCanvas`.
#[component]
fn ProjectMapSection(
    project_id: Uuid,
    plots: Vec<PlotWithColor>,
    selected: RwSignal<Option<PlotWithColor>>,
) -> impl IntoView {
    let api = use_api();
    let refresh = RwSignal::new(0u32);

    let summary = LocalResource::new({
        let api = api.clone();
        move || {
            let api = api.clone();
            refresh.get();
            async move { api.get_map_summary(project_id).await }
        }
    });

    view! {
        <Suspense fallback=|| view! { <LoadingState label="Loading map…" /> }>
            {move || {
                let plots = plots.clone();
                summary
                    .get()
                    .map(|wrapped| wrapped.take())
                    .map(|result| match result {
                        Ok(s) if !s.exists => view! {
                            <MapUploadForm
                                project_id=project_id
                                on_uploaded=move || refresh.update(|n| *n += 1)
                            />
                        }
                            .into_any(),
                        Ok(s) => view! {
                            <MapCanvas
                                project_id=project_id
                                plots=plots
                                summary=s
                                selected=selected
                                refresh=refresh
                            />
                        }
                            .into_any(),
                        Err(e) => view! { <ErrorAlert message=format!("Couldn't load the map: {e}") /> }
                            .into_any(),
                    })
            }}
        </Suspense>
    }
}

#[component]
fn MapUploadForm(project_id: Uuid, on_uploaded: impl Fn() + Clone + 'static) -> impl IntoView {
    let api = use_api();
    let auth = use_auth();
    let can_upload = has_permission(auth, PERM_PLOTS_MAP_UPLOAD);
    let error = RwSignal::new(None::<String>);
    let uploading = RwSignal::new(false);

    let on_change = move |ev: leptos::ev::Event| {
        let Some(input) = ev
            .target()
            .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
        else {
            return;
        };
        let Some(files) = input.files() else { return };
        let Some(file) = files.item(0) else { return };
        if uploading.get() {
            return;
        }
        error.set(None);
        uploading.set(true);
        let api = api.clone();
        let on_uploaded = on_uploaded.clone();
        spawn_local(async move {
            match api.upload_map_image(project_id, file).await {
                Ok(_) => on_uploaded(),
                Err(e) => error.set(Some(format!("{e}"))),
            }
            uploading.set(false);
        });
    };

    view! {
        <EmptyState
            icon="\u{1F5FA}\u{FE0F}"
            title="No site plan uploaded yet"
            detail="Upload an image of this project's site plan, then draw each plot's boundary on top of it."
        />
        {move || error.get().map(|msg| view! { <ErrorAlert message=msg /> })}
        {if can_upload {
            view! {
                <div class="field" style="max-width: 360px;">
                    <label for="map-image">
                        {move || if uploading.get() { "Uploading…" } else { "Site plan image" }}
                    </label>
                    <input id="map-image" type="file" accept="image/*" disabled=uploading on:change=on_change />
                </div>
            }.into_any()
        } else {
            view! { <p class="meta">"You don't have permission to upload a site plan — ask an admin."</p> }.into_any()
        }}
    }
}

/// A single project's map: the uploaded image with an SVG polygon
/// overlay. Coordinates are pixels against the image's natural size,
/// not geographic — see `domain::MapPolygons`'s module docs. Outside
/// edit mode, clicking a *linked* polygon selects that plot (reusing
/// the same `selected` signal — and so the same Reserve/Quote card —
/// as clicking a tile in the grid view); clicking an *unlinked* (draft)
/// polygon instead opens `selected_draft`'s "Create Plot"/"Link
/// Existing Plot" panel below the map, this component's core addition
/// — a shape no longer needs a plot picked before it can be drawn (see
/// `domain::MapFeature`'s module docs for why). In edit mode, clicking
/// the image places a boundary point and clicking an existing polygon
/// (draft or linked) removes it.
#[component]
fn MapCanvas(
    project_id: Uuid,
    plots: Vec<PlotWithColor>,
    summary: domain::ProjectMapSummary,
    selected: RwSignal<Option<PlotWithColor>>,
    refresh: RwSignal<u32>,
) -> impl IntoView {
    let api = use_api();
    let auth = use_auth();
    let can_edit_boundaries = has_permission(auth, PERM_PLOTS_MAP_EDIT_BOUNDARIES);
    let can_link = has_permission(auth, PERM_PLOTS_MAP_LINK);
    let image_url = api.map_image_url(project_id);

    let img_dims = RwSignal::new((
        if summary.polygons.image_width > 0.0 { summary.polygons.image_width } else { 1000.0 },
        if summary.polygons.image_height > 0.0 { summary.polygons.image_height } else { 700.0 },
    ));
    let features: RwSignal<Vec<MapFeature>> = RwSignal::new(summary.polygons.features.clone());
    let edit_mode = RwSignal::new(false);
    let draft_points: RwSignal<Vec<(f64, f64)>> = RwSignal::new(Vec::new());
    let draft_label = RwSignal::new(String::new());
    let selected_draft: RwSignal<Option<MapFeature>> = RwSignal::new(None);
    let plots_for_draft_panel = plots.clone();
    let error = RwSignal::new(None::<String>);
    let saving = RwSignal::new(false);
    let replacing = RwSignal::new(false);

    let on_image_load = move |ev: leptos::ev::Event| {
        if let Some(img) = ev
            .target()
            .and_then(|t| t.dyn_into::<web_sys::HtmlImageElement>().ok())
        {
            let w = img.natural_width() as f64;
            let h = img.natural_height() as f64;
            if w > 0.0 && h > 0.0 {
                img_dims.set((w, h));
            }
        }
    };

    let on_svg_click = move |ev: leptos::ev::MouseEvent| {
        if !edit_mode.get() {
            return;
        }
        let Some(target) = ev.current_target() else { return };
        let Ok(el) = target.dyn_into::<web_sys::Element>() else { return };
        let rect = el.get_bounding_client_rect();
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            return;
        }
        let (w, h) = img_dims.get();
        let x = (ev.client_x() as f64 - rect.left()) / rect.width() * w;
        let y = (ev.client_y() as f64 - rect.top()) / rect.height() * h;
        draft_points.update(|pts| pts.push((x, y)));
    };

    let api_for_replace = api.clone();
    let on_replace_change = move |ev: leptos::ev::Event| {
        let Some(input) = ev
            .target()
            .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
        else {
            return;
        };
        let Some(files) = input.files() else { return };
        let Some(file) = files.item(0) else { return };
        if replacing.get() {
            return;
        }
        error.set(None);
        replacing.set(true);
        let api = api_for_replace.clone();
        spawn_local(async move {
            match api.upload_map_image(project_id, file).await {
                Ok(_) => refresh.update(|n| *n += 1),
                Err(e) => error.set(Some(format!("{e}"))),
            }
            replacing.set(false);
        });
    };

    view! {
        {move || error.get().map(|msg| view! { <ErrorAlert message=msg /> })}

        <div style="display:flex; justify-content: space-between; align-items:center; gap: var(--space-3); flex-wrap: wrap; margin-bottom: var(--space-2);">
            <p class="meta mt-0">
                {move || if edit_mode.get() {
                    "Click the image to place boundary points; click an existing shape to remove it."
                } else {
                    "Click a shape to view its plot — or, if it isn't linked to one yet, to create or link one."
                }}
            </p>
            <div style="display:flex; gap: var(--space-2); align-items:center;">
                {can_edit_boundaries.then(|| view! {
                    <label class="btn btn-secondary" style="margin-bottom:0; cursor:pointer;">
                        {move || if replacing.get() { "Replacing…" } else { "Replace image" }}
                        <input
                            type="file"
                            accept="image/*"
                            disabled=replacing
                            style="display:none;"
                            on:change=on_replace_change
                        />
                    </label>
                })}
                {can_edit_boundaries.then(|| view! {
                    <button
                        type="button"
                        class="btn btn-secondary"
                        on:click=move |_| {
                            edit_mode.update(|v| *v = !*v);
                            draft_points.set(Vec::new());
                        }
                    >
                        {move || if edit_mode.get() { "Done editing" } else { "Edit boundaries" }}
                    </button>
                })}
            </div>
        </div>

        <Show when=move || edit_mode.get() && can_edit_boundaries>
            {
                // Fresh per-invocation clone: `<Show>`'s children run
                // repeatedly as `edit_mode` toggles, but this outer
                // `api` is captured once — see the identical note in
                // `pages/quotation_detail.rs::run`.
                let api = api.clone();
                view! {
            <div class="card" style="margin-bottom: var(--space-3); display:flex; gap: var(--space-3); align-items:flex-end; flex-wrap:wrap;">
                <div class="field" style="margin-bottom:0;">
                    <label for="draft-label">"Label (optional)"</label>
                    <input
                        id="draft-label"
                        type="text"
                        placeholder="e.g. Lot 1"
                        prop:value=draft_label
                        on:input=move |ev| draft_label.set(event_target_value(&ev))
                    />
                </div>
                <span class="meta">{move || format!("{} point(s) placed", draft_points.get().len())}</span>
                <button
                    type="button"
                    class="btn btn-primary"
                    on:click=move |_| {
                        let pts = draft_points.get();
                        if pts.len() < 3 {
                            error.set(Some("Click at least 3 points to outline a plot.".to_string()));
                            return;
                        }
                        error.set(None);
                        let label = draft_label.get();
                        let label = (!label.trim().is_empty()).then(|| label.trim().to_string());
                        features.update(|list| {
                            list.push(MapFeature {
                                id: format!("f-{}", Uuid::new_v4()),
                                plot_id: None,
                                label,
                                points: pts.iter().map(|&(x, y)| [x, y]).collect(),
                            });
                        });
                        draft_points.set(Vec::new());
                        draft_label.set(String::new());
                    }
                >
                    "Finish shape"
                </button>
                <button
                    type="button"
                    class="btn btn-secondary"
                    on:click=move |_| draft_points.set(Vec::new())
                >
                    "Clear points"
                </button>
                <button
                    type="button"
                    class="btn btn-primary"
                    disabled=saving
                    on:click=move |_| {
                        if saving.get() {
                            return;
                        }
                        error.set(None);
                        saving.set(true);
                        let api = api.clone();
                        let (image_width, image_height) = img_dims.get();
                        let polygons = MapPolygons { image_width, image_height, features: features.get() };
                        spawn_local(async move {
                            match api.update_map_polygons(project_id, polygons).await {
                                Ok(_) => refresh.update(|n| *n += 1),
                                Err(crate::api::ApiError::InvalidCredentials(msg)) => error.set(Some(msg)),
                                Err(e) => error.set(Some(format!("{e}"))),
                            }
                            saving.set(false);
                        });
                    }
                >
                    {move || if saving.get() { "Saving…" } else { "Save map" }}
                </button>
            </div>
                }
            }
        </Show>

        <div style="position:relative; max-width: 100%; display:inline-block; line-height:0;">
            <img
                src=image_url
                on:load=on_image_load
                style="display:block; max-width:100%; height:auto; border-radius: var(--radius, 8px);"
            />
            <svg
                on:click=on_svg_click
                viewBox=move || {
                    let (w, h) = img_dims.get();
                    format!("0 0 {w} {h}")
                }
                style="position:absolute; top:0; left:0; width:100%; height:100%;"
            >
                {move || {
                    let plots = plots.clone();
                    features
                        .get()
                        .into_iter()
                        .map(|f| {
                            let points_attr = f
                                .points
                                .iter()
                                .map(|[x, y]| format!("{x},{y}"))
                                .collect::<Vec<_>>()
                                .join(" ");
                            let color = feature_color(&plots, f.plot_id);
                            let fid = f.id.clone();
                            let plot_for_select = f.plot_id.and_then(|id| plots.iter().find(|p| p.plot.id == id)).cloned();
                            let feature_for_draft = f.clone();
                            let (cx, cy) = polygon_centroid(&f.points);
                            let label = f.label.clone();
                            // Draft shapes get a dashed outline — visually
                            // distinct from a real, coloured-by-status plot
                            // (`domain::plot_status_meta`'s palette never
                            // produces this neutral grey) so nobody mistakes
                            // an unconfigured shape for an available one.
                            let dash = if f.plot_id.is_none() { "6,4" } else { "" };
                            view! {
                                <polygon
                                    points=points_attr
                                    fill=format!("{color}99")
                                    stroke=color.clone()
                                    stroke-width="2"
                                    stroke-dasharray=dash
                                    style="cursor:pointer;"
                                    on:click=move |ev: leptos::ev::MouseEvent| {
                                        ev.stop_propagation();
                                        if edit_mode.get() {
                                            let fid = fid.clone();
                                            features.update(|list| list.retain(|x| x.id != fid));
                                        } else if let Some(p) = plot_for_select.clone() {
                                            selected.set(Some(p));
                                        } else {
                                            selected_draft.set(Some(feature_for_draft.clone()));
                                        }
                                    }
                                ></polygon>
                                {label.map(|text| view! {
                                    <text
                                        x=cx.to_string()
                                        y=cy.to_string()
                                        fill="#fff"
                                        stroke="#00000099"
                                        stroke-width="3"
                                        paint-order="stroke"
                                        font-size="13"
                                        font-weight="700"
                                        text-anchor="middle"
                                        dominant-baseline="middle"
                                        style="pointer-events:none;"
                                    >
                                        {text}
                                    </text>
                                })}
                            }
                        })
                        .collect_view()
                }}

                {move || {
                    let pts = draft_points.get();
                    if pts.is_empty() {
                        None
                    } else {
                        let points_attr = pts.iter().map(|(x, y)| format!("{x},{y}")).collect::<Vec<_>>().join(" ");
                        Some(view! {
                            <polyline
                                points=points_attr
                                fill="none"
                                stroke="#f97316"
                                stroke-width="3"
                                stroke-dasharray="6,4"
                            ></polyline>
                        })
                    }
                }}
            </svg>
        </div>

        {move || {
            selected_draft.get().map(|feature| {
                let unlinked_plots: Vec<PlotWithColor> = {
                    let linked_ids: std::collections::HashSet<Uuid> =
                        features.get().iter().filter_map(|f| f.plot_id).collect();
                    plots_for_draft_panel.iter().filter(|p| !linked_ids.contains(&p.plot.id)).cloned().collect()
                };
                let show_link = RwSignal::new(false);
                let feature_id = feature.id.clone();
                let label_text = feature.label
                    .clone()
                    .filter(|l| !l.trim().is_empty())
                    .unwrap_or_else(|| "Unnamed shape".to_string());
                view! {
                    <div class="card" style="margin-top: var(--space-4)">
                        <div class="page-header" style="margin-bottom: var(--space-3)">
                            <h2 class="mt-0">{label_text}</h2>
                            <span class="meta">"Not yet linked to a plot"</span>
                        </div>
                        {if !can_link {
                            view! { <p class="meta">"You don't have permission to link plots to map shapes — ask an admin."</p> }.into_any()
                        } else { view! {
                        <div class="filter-tabs">
                            <button
                                type="button"
                                class="filter-tab"
                                class:active=move || !show_link.get()
                                on:click=move |_| show_link.set(false)
                            >
                                "Create plot"
                            </button>
                            <button
                                type="button"
                                class="filter-tab"
                                class:active=move || show_link.get()
                                on:click=move |_| show_link.set(true)
                            >
                                "Link existing plot"
                            </button>
                        </div>
                        {move || {
                            let feature_id = feature_id.clone();
                            let unlinked_plots = unlinked_plots.clone();
                            if show_link.get() {
                                view! {
                                    <LinkExistingPlotForm
                                        project_id=project_id
                                        feature_id=feature_id
                                        unlinked_plots=unlinked_plots
                                        on_linked=move || {
                                            selected_draft.set(None);
                                            refresh.update(|n| *n += 1);
                                        }
                                    />
                                }
                                    .into_any()
                            } else {
                                view! {
                                    <CreatePlotFromMapForm
                                        project_id=project_id
                                        feature_id=feature_id
                                        on_created=move || {
                                            selected_draft.set(None);
                                            refresh.update(|n| *n += 1);
                                        }
                                    />
                                }
                                    .into_any()
                            }
                        }}
                        }.into_any() }}
                        <button
                            class="btn btn-secondary"
                            style="margin-top: var(--space-3);"
                            on:click=move |_| selected_draft.set(None)
                        >
                            "Close"
                        </button>
                    </div>
                }
            })
        }}
    }
}

#[component]
fn CreatePlotFromMapForm(
    project_id: Uuid,
    feature_id: String,
    on_created: impl Fn() + Clone + 'static,
) -> impl IntoView {
    let api = use_api();
    let currency = use_currency();

    let plot_number = RwSignal::new(String::new());
    let size = RwSignal::new(String::new());
    let side_1 = RwSignal::new(String::new());
    let side_2 = RwSignal::new(String::new());
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
        let parsed_side_1 = match parse_optional_dimension(&side_1.get()) {
            Ok(v) => v,
            Err(msg) => {
                error.set(Some(msg));
                return;
            }
        };
        let parsed_side_2 = match parse_optional_dimension(&side_2.get()) {
            Ok(v) => v,
            Err(msg) => {
                error.set(Some(msg));
                return;
            }
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
        let on_created = on_created.clone();
        let feature_id = feature_id.clone();
        let input = CreatePlotInput {
            project_id,
            plot_number: plot_number.get(),
            size: parsed_size,
            side_1: parsed_side_1,
            side_2: parsed_side_2,
            asking_price: parsed_asking,
            minimum_price: parsed_minimum,
        };
        spawn_local(async move {
            match api.create_plot_for_map_feature(project_id, &feature_id, input).await {
                Ok(_) => {
                    // `on_created` unmounts this component (it clears the
                    // selected draft), which disposes `submitting`/`error` —
                    // so touch them first, before disposal, never after.
                    submitting.set(false);
                    on_created();
                }
                Err(e) => {
                    error.set(Some(format!("{e}")));
                    submitting.set(false);
                }
            }
        });
    };

    view! {
        <form on:submit=on_submit>
            {move || error.get().map(|msg| view! { <ErrorAlert message=msg /> })}

            <div class="field">
                <label for="map-plot-number">"Plot number"</label>
                <input
                    id="map-plot-number"
                    type="text"
                    required
                    prop:value=plot_number
                    on:input=move |ev| plot_number.set(event_target_value(&ev))
                />
            </div>

            <div class="field">
                <label for="map-plot-size">"Size (acres)"</label>
                <input
                    id="map-plot-size"
                    type="text"
                    inputmode="decimal"
                    required
                    prop:value=size
                    on:input=move |ev| size.set(event_target_value(&ev))
                />
            </div>

            <div class="field">
                <label>"Plot dimensions (feet)"</label>
                <div class="dimension-row">
                    <span class="dimension-label">"Side 1"</span>
                    <input
                        type="text"
                        inputmode="decimal"
                        placeholder="80"
                        aria-label="Side 1"
                        prop:value=side_1
                        on:input=move |ev| side_1.set(event_target_value(&ev))
                    />
                    <span class="dimension-label">"by"</span>
                    <span class="dimension-label">"Side 2"</span>
                    <input
                        type="text"
                        inputmode="decimal"
                        placeholder="100"
                        aria-label="Side 2"
                        prop:value=side_2
                        on:input=move |ev| side_2.set(event_target_value(&ev))
                    />
                    <span class="dimension-label">"ft"</span>
                </div>
            </div>

            <div class="field">
                <label for="map-asking-price">"Asking price (" {move || currency.get()} ")"</label>
                <input
                    id="map-asking-price"
                    type="text"
                    inputmode="numeric"
                    required
                    prop:value=asking_price
                    on:input=move |ev| asking_price.set(event_target_value(&ev))
                />
            </div>

            <div class="field">
                <label for="map-minimum-price">"Minimum price (" {move || currency.get()} ", optional)"</label>
                <input
                    id="map-minimum-price"
                    type="text"
                    inputmode="numeric"
                    prop:value=minimum_price
                    on:input=move |ev| minimum_price.set(event_target_value(&ev))
                />
            </div>

            <button type="submit" class="btn btn-primary" disabled=submitting>
                {move || if submitting.get() { "Creating…" } else { "Create plot" }}
            </button>
        </form>
    }
}

#[component]
fn LinkExistingPlotForm(
    project_id: Uuid,
    feature_id: String,
    unlinked_plots: Vec<PlotWithColor>,
    on_linked: impl Fn() + Clone + 'static,
) -> impl IntoView {
    let api = use_api();
    let chosen = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);
    let submitting = RwSignal::new(false);

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if submitting.get() {
            return;
        }
        error.set(None);
        let Ok(plot_id) = Uuid::parse_str(chosen.get().trim()) else {
            error.set(Some("Choose a plot.".to_string()));
            return;
        };
        submitting.set(true);
        let api = api.clone();
        let on_linked = on_linked.clone();
        let feature_id = feature_id.clone();
        spawn_local(async move {
            match api.link_plot_to_map_feature(project_id, &feature_id, plot_id).await {
                Ok(_) => {
                    // Same disposal-order hazard as CreatePlotFromMapForm:
                    // `on_linked` unmounts this component, so update
                    // `submitting` before it, never after.
                    submitting.set(false);
                    on_linked();
                }
                Err(e) => {
                    error.set(Some(format!("{e}")));
                    submitting.set(false);
                }
            }
        });
    };

    view! {
        <form on:submit=on_submit>
            {move || error.get().map(|msg| view! { <ErrorAlert message=msg /> })}

            {if unlinked_plots.is_empty() {
                view! { <p class="meta">"No unlinked plots in this project to link."</p> }.into_any()
            } else {
                view! {
                    <div class="field">
                        <label for="link-plot-select">"Plot"</label>
                        <select
                            id="link-plot-select"
                            required
                            prop:value=chosen
                            on:change=move |ev| chosen.set(event_target_value(&ev))
                        >
                            <option value="">"Select a plot…"</option>
                            {unlinked_plots.iter().map(|p| {
                                let id = p.plot.id.to_string();
                                view! { <option value=id>{p.plot.plot_number.clone()}</option> }
                            }).collect_view()}
                        </select>
                    </div>
                }
                    .into_any()
            }}

            <button type="submit" class="btn btn-primary" disabled=submitting>
                {move || if submitting.get() { "Linking…" } else { "Link plot" }}
            </button>
        </form>
    }
}
