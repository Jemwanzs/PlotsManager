mod api;
mod app;
mod auth;
mod components;
mod csv_import;
mod format;
mod icons;
mod layout;
mod pages;
mod theme;

use app::App;

fn main() {
    console_error_panic_hook::set_once();
    _ = console_log::init_with_level(log::Level::Debug);
    leptos::mount::mount_to_body(App);
}
