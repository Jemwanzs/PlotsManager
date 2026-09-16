use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::wasm_bindgen::JsCast;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;
use rust_decimal::Decimal;
use std::str::FromStr;
use uuid::Uuid;

use crate::api::{status_meta, CreatePlotInput, CreateQuotationInput, CreateSaleInput, PlotWithColor};
use crate::auth::{use_api, use_currency};
use crate::components::{EmptyState, ErrorAlert, LoadingState, StatusBadge};
use crate::csv_import::{self, ParsedRow};
use crate::format::format_money;
use domain::{MapFeature, MapPolygons, PaymentMode, PlotStatus};

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
fn feature_color(plots: &[PlotWithColor], plot_id: Uuid) -> String {
    plots
        .iter()
        .find(|p| p.plot.id == plot_id)
        .map(|p| p.status_color.clone())
        .unwrap_or_else(|| "#6b7280".to_string())
}

#[component]
pub fn ProjectDetail() -> impl IntoView {
    let api = use_api();
    let currency = use_currency();
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
                                        <button
                                            class="btn btn-secondary"
                                            on:click=move |_| {
                                                show_bulk_import.update(|v| *v = !*v);
                                                show_add_plot.set(false);
                                            }
                                        >
                                            {move || if show_bulk_import.get() { "Cancel" } else { "Bulk import" }}
                                        </button>
                                        <button
                                            class="btn btn-secondary"
                                            on:click=move |_| {
                                                show_add_plot.update(|v| *v = !*v);
                                                show_bulk_import.set(false);
                                            }
                                        >
                                            {move || if show_add_plot.get() { "Cancel" } else { "+ Add plot" }}
                                        </button>
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
                        Ok(list) if show_map.get() => {
                            let Some(id) = project_id() else { return ().into_any() };
                            view! { <ProjectMapSection project_id=id plots=list selected=selected /> }
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
                            {pwc.plot.title_number.clone().map(|t| view! { <p>"Title: " {t}</p> })}

                            <Show when=move || startable>
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

                                <Show
                                    when=move || show_quote_form.get()
                                    fallback=move || view! {
                                        <ReserveForm
                                            plot_id=plot_id
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
                        {move || if generating.get() { "…" } else { "Auto-generate" }}
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
            <p class="meta mt-0">"CSV columns, in order: plot_number, size, asking_price, minimum_price."</p>
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
        <div class="field" style="max-width: 360px;">
            <label for="map-image">
                {move || if uploading.get() { "Uploading…" } else { "Site plan image" }}
            </label>
            <input id="map-image" type="file" accept="image/*" disabled=uploading on:change=on_change />
        </div>
    }
}

/// A single project's map: the uploaded image with an SVG polygon
/// overlay. Coordinates are pixels against the image's natural size,
/// not geographic — see `domain::MapPolygons`'s module docs. Outside
/// edit mode, clicking a polygon selects that plot (reusing the same
/// `selected` signal — and so the same Reserve/Quote card — as
/// clicking a tile in the grid view); in edit mode, clicking the image
/// places a boundary point and clicking an existing polygon removes it.
#[component]
fn MapCanvas(
    project_id: Uuid,
    plots: Vec<PlotWithColor>,
    summary: domain::ProjectMapSummary,
    selected: RwSignal<Option<PlotWithColor>>,
    refresh: RwSignal<u32>,
) -> impl IntoView {
    let api = use_api();
    let image_url = api.map_image_url(project_id);

    let img_dims = RwSignal::new((
        if summary.polygons.image_width > 0.0 { summary.polygons.image_width } else { 1000.0 },
        if summary.polygons.image_height > 0.0 { summary.polygons.image_height } else { 700.0 },
    ));
    let features: RwSignal<Vec<MapFeature>> = RwSignal::new(summary.polygons.features.clone());
    let edit_mode = RwSignal::new(false);
    let draft_points: RwSignal<Vec<(f64, f64)>> = RwSignal::new(Vec::new());
    let draft_plot_id = RwSignal::new(String::new());
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

    let plots_for_dropdown = plots.clone();

    view! {
        {move || error.get().map(|msg| view! { <ErrorAlert message=msg /> })}

        <div style="display:flex; justify-content: space-between; align-items:center; gap: var(--space-3); flex-wrap: wrap; margin-bottom: var(--space-2);">
            <p class="meta mt-0">
                {move || if edit_mode.get() {
                    "Click the image to place boundary points; click a shaded plot to remove it."
                } else {
                    "Click a shaded plot to view it."
                }}
            </p>
            <div style="display:flex; gap: var(--space-2); align-items:center;">
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
            </div>
        </div>

        <Show when=move || edit_mode.get()>
            {
                // Fresh per-invocation clone: `<Show>`'s children run
                // repeatedly as `edit_mode` toggles, but this outer
                // `api` is captured once — see the identical note in
                // `pages/quotation_detail.rs::run`.
                let api = api.clone();
                view! {
            <div class="card" style="margin-bottom: var(--space-3); display:flex; gap: var(--space-3); align-items:flex-end; flex-wrap:wrap;">
                <div class="field" style="margin-bottom:0;">
                    <label for="draft-plot">"Plot for next shape"</label>
                    <select
                        id="draft-plot"
                        prop:value=draft_plot_id
                        on:change=move |ev| draft_plot_id.set(event_target_value(&ev))
                    >
                        <option value="">"Select a plot…"</option>
                        {plots_for_dropdown
                            .iter()
                            .map(|p| {
                                let id = p.plot.id.to_string();
                                view! { <option value=id>{p.plot.plot_number.clone()}</option> }
                            })
                            .collect_view()}
                    </select>
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
                        let Ok(plot_id) = Uuid::parse_str(draft_plot_id.get().trim()) else {
                            error.set(Some("Choose which plot this shape is for.".to_string()));
                            return;
                        };
                        error.set(None);
                        features.update(|list| {
                            list.retain(|f| f.plot_id != plot_id);
                            list.push(MapFeature {
                                id: format!("f-{}", Uuid::new_v4()),
                                plot_id,
                                points: pts.iter().map(|&(x, y)| [x, y]).collect(),
                            });
                        });
                        draft_points.set(Vec::new());
                        draft_plot_id.set(String::new());
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
                attr:viewBox=move || {
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
                            let plot_for_select = plots.iter().find(|p| p.plot.id == f.plot_id).cloned();
                            view! {
                                <polygon
                                    points=points_attr
                                    fill=format!("{color}99")
                                    stroke=color.clone()
                                    stroke-width="2"
                                    style="cursor:pointer;"
                                    on:click=move |ev: leptos::ev::MouseEvent| {
                                        ev.stop_propagation();
                                        if edit_mode.get() {
                                            let fid = fid.clone();
                                            features.update(|list| list.retain(|x| x.id != fid));
                                        } else if let Some(p) = plot_for_select.clone() {
                                            selected.set(Some(p));
                                        }
                                    }
                                ></polygon>
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
    }
}
