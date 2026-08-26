use domain::PlotStatus;
use leptos::prelude::*;

// Not yet called from any component — the auth/CRUD screens that will use
// it are Phase 2 UI work, not yet built. Silence dead_code until then
// rather than leaving real warnings that would mask new ones.
#[allow(dead_code)]
mod supabase;

fn main() {
    console_error_panic_hook::set_once();
    _ = console_log::init_with_level(log::Level::Debug);
    leptos::mount::mount_to_body(App);
}

/// Placeholder shell. Real screens (project map, plot inventory, sales
/// dashboards) land in Phase 2/3 of the roadmap — see /docs.
#[component]
fn App() -> impl IntoView {
    let statuses: Vec<PlotStatus> = vec![
        PlotStatus::Available,
        PlotStatus::Reserved,
        PlotStatus::Booked,
        PlotStatus::Sold,
    ];

    view! {
        <main style="font-family: sans-serif; padding: 2rem;">
            <h1>"Real Estate Manager"</h1>
            <p>"Frontend scaffold is up. Plot status enum shared from the domain crate:"</p>
            <ul>
                {statuses
                    .into_iter()
                    .map(|s| view! { <li>{format!("{s:?}")}</li> })
                    .collect_view()}
            </ul>
        </main>
    }
}
