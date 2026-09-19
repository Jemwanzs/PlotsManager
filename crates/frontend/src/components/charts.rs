//! Small, dependency-free chart primitives for the executive dashboard
//! (`crates/frontend/src/pages/dashboard.rs`) — plain inline SVG (line,
//! donut) or plain divs (bar), matching this app's "no framework, no
//! external JS" convention rather than pulling in a charting library
//! for a Trunk/WASM build. One axis per chart, a fixed color per
//! series/segment (never re-derived per render), a legend wherever
//! there's more than one series, and a hover tooltip on the line chart
//! since a trend line's individual points aren't otherwise readable.

use leptos::prelude::*;

/// One point on a line chart or one bar on a bar chart — `value` drives
/// the geometry, `display` is what a human reads (already formatted
/// with currency/commas by the caller, which is the one place that
/// knows the org's currency).
#[derive(Debug, Clone)]
pub struct ChartPoint {
    pub label: String,
    pub value: f64,
    pub display: String,
}

/// One wedge of a donut chart. `color` is a fixed hex the caller
/// supplies (this app's existing `plot_status_meta` palette for the
/// inventory-by-status chart) — never generated here, so the same
/// status always reads as the same color everywhere in the app.
#[derive(Debug, Clone)]
pub struct DonutSegment {
    pub label: String,
    pub value: f64,
    pub display: String,
    pub color: String,
}

/// A single-series trend line with a light area fill under it and a
/// hover tooltip. `points` is assumed already in x-order (oldest
/// first) — this component only draws, it doesn't sort.
#[component]
pub fn LineChart(points: Vec<ChartPoint>) -> impl IntoView {
    const WIDTH: f64 = 640.0;
    const HEIGHT: f64 = 220.0;
    const PAD_LEFT: f64 = 6.0;
    const PAD_RIGHT: f64 = 6.0;
    const PAD_TOP: f64 = 14.0;
    const PAD_BOTTOM: f64 = 8.0;
    const PLOT_W: f64 = WIDTH - PAD_LEFT - PAD_RIGHT;
    const PLOT_H: f64 = HEIGHT - PAD_TOP - PAD_BOTTOM;

    let max_v = points.iter().map(|p| p.value).fold(0.0_f64, f64::max).max(1.0);
    let n = points.len().max(1);
    let step = if n > 1 { PLOT_W / (n as f64 - 1.0) } else { 0.0 };

    let coords: Vec<(f64, f64)> = points
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let x = PAD_LEFT + step * i as f64;
            let y = PAD_TOP + PLOT_H - (p.value / max_v) * PLOT_H;
            (x, y)
        })
        .collect();

    let line_d = coords
        .iter()
        .enumerate()
        .map(|(i, (x, y))| if i == 0 { format!("M {x:.1} {y:.1}") } else { format!("L {x:.1} {y:.1}") })
        .collect::<Vec<_>>()
        .join(" ");
    let baseline_y = PAD_TOP + PLOT_H;
    let area_d = match (coords.first(), coords.last()) {
        (Some((x0, _)), Some((x1, _))) => {
            format!("{line_d} L {x1:.1} {baseline_y:.1} L {x0:.1} {baseline_y:.1} Z")
        }
        _ => String::new(),
    };

    let hovered: RwSignal<Option<usize>> = RwSignal::new(None);
    let points_for_tooltip = points.clone();
    let points_for_labels = points.clone();

    view! {
        <div class="chart-wrap">
            <svg
                attr:viewBox=format!("0 0 {WIDTH} {HEIGHT}")
                attr:preserveAspectRatio="none"
                class="chart-svg line-chart-svg"
            >
                <line
                    x1=PAD_LEFT y1=baseline_y x2=WIDTH - PAD_RIGHT y2=baseline_y
                    stroke="var(--color-border)" stroke-width="1"
                />
                {if area_d.is_empty() { ().into_any() } else {
                    view! { <path d=area_d fill="var(--color-primary)" opacity="0.12" stroke="none" /> }.into_any()
                }}
                <path d=line_d fill="none" stroke="var(--color-primary)" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" />
                {coords.iter().copied().enumerate().map(|(i, (x, y))| {
                    let is_hovered = move || hovered.get() == Some(i);
                    view! {
                        <circle
                            cx=x cy=y r="3.5"
                            fill=move || if is_hovered() { "var(--color-primary)" } else { "var(--color-surface)" }
                            stroke="var(--color-primary)" stroke-width="2"
                        />
                        <circle
                            cx=x cy=y r="12" fill="transparent" class="chart-hit"
                            on:mouseenter=move |_| hovered.set(Some(i))
                            on:mouseleave=move |_| hovered.set(None)
                        />
                    }
                }).collect_view()}
            </svg>
            <div class="chart-x-axis">
                {points_for_labels.into_iter().enumerate().map(|(i, p)| {
                    view! {
                        <span class="chart-x-label" class:active=move || hovered.get() == Some(i)>{p.label}</span>
                    }
                }).collect_view()}
            </div>
            <div class="chart-tooltip" class:visible=move || hovered.get().is_some()>
                {move || hovered.get().map(|i| {
                    let p = &points_for_tooltip[i];
                    view! {
                        <strong>{p.label.clone()}</strong>
                        <span>{p.display.clone()}</span>
                    }.into_any()
                })}
            </div>
        </div>
    }
}

/// Inventory-by-status donut, drawn as stacked circle strokes (each
/// segment its own `stroke-dasharray`/`stroke-dashoffset` slice of the
/// same circle) rather than hand-computed SVG arc paths — the standard
/// lightweight way to fake a donut without arc-flag math.
#[component]
pub fn DonutChart(segments: Vec<DonutSegment>, center_label: String) -> impl IntoView {
    const RADIUS: f64 = 64.0;
    const STROKE_WIDTH: f64 = 26.0;
    const GAP: f64 = 3.0;
    let size = (RADIUS + STROKE_WIDTH) * 2.0;
    let center = size / 2.0;
    let circumference = 2.0 * std::f64::consts::PI * RADIUS;

    let total: f64 = segments.iter().map(|s| s.value).sum::<f64>().max(0.0001);

    let mut cumulative = 0.0_f64;
    let arcs: Vec<(String, f64, f64, String)> = segments
        .iter()
        .map(|s| {
            let raw_len = (s.value / total) * circumference;
            let seg_len = (raw_len - GAP).max(0.0);
            let offset = -cumulative;
            cumulative += raw_len;
            (s.label.clone(), seg_len, offset, s.color.clone())
        })
        .collect();

    let legend = segments.clone();

    view! {
        <div class="donut-wrap">
            <svg attr:viewBox=format!("0 0 {size} {size}") class="donut-svg">
                <g attr:transform=format!("rotate(-90 {center} {center})")>
                    {arcs.into_iter().map(|(label, seg_len, offset, color)| {
                        view! {
                            <circle
                                cx=center cy=center r=RADIUS fill="none"
                                stroke=color stroke-width=STROKE_WIDTH
                                stroke-dasharray=format!("{seg_len:.2} {:.2}", circumference - seg_len)
                                stroke-dashoffset=format!("{offset:.2}")
                                attr:aria-label=label
                            />
                        }
                    }).collect_view()}
                </g>
                <text
                    x=center y=center - 4.0
                    text-anchor="middle" class="donut-center-value"
                    attr:fill="var(--color-text)"
                >
                    {center_label.clone()}
                </text>
                <text
                    x=center y=center + 16.0
                    text-anchor="middle" class="donut-center-caption"
                    attr:fill="var(--color-text-muted)"
                >
                    "total plots"
                </text>
            </svg>
            <ul class="chart-legend">
                {legend.into_iter().map(|s| {
                    view! {
                        <li>
                            <span class="legend-swatch" style=format!("background-color: {}", s.color)></span>
                            <span class="legend-label">{s.label}</span>
                            <span class="legend-value">{s.display}</span>
                        </li>
                    }
                }).collect_view()}
            </ul>
        </div>
    }
}

/// Horizontal bars, one per category, sorted by the caller. A single
/// fixed hue (magnitude, not identity) with every value directly
/// labeled — with this few bars (top ~8 projects) there's no need for
/// hover-only labels.
#[component]
pub fn BarChart(bars: Vec<ChartPoint>) -> impl IntoView {
    let max_v = bars.iter().map(|b| b.value).fold(0.0_f64, f64::max).max(1.0);

    view! {
        <div class="bar-chart">
            {bars.into_iter().map(|b| {
                let pct = ((b.value / max_v) * 100.0).clamp(2.0, 100.0);
                view! {
                    <div class="bar-row">
                        <span class="bar-label">{b.label}</span>
                        <div class="bar-track">
                            <div class="bar-fill" style=format!("width: {pct:.1}%")></div>
                        </div>
                        <span class="bar-value">{b.display}</span>
                    </div>
                }
            }).collect_view()}
        </div>
    }
}
