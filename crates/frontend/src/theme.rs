//! Light/dark mode. The actual colours live entirely in CSS custom
//! properties (`styles/app.css`'s `:root` / `:root[data-theme="dark"]` /
//! the `prefers-color-scheme` fallback for a visitor who's never
//! toggled) — this module's only job is deciding which one is active
//! and keeping `<html data-theme="...">` and `localStorage` in sync with
//! it, so a toggle click is instant (no re-render of anything but the
//! custom properties themselves) and sticks across visits.

use leptos::prelude::*;

pub type ThemeSignal = RwSignal<Theme>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Light,
    Dark,
}

impl Theme {
    fn as_attr(self) -> &'static str {
        match self {
            Theme::Light => "light",
            Theme::Dark => "dark",
        }
    }

    pub fn toggled(self) -> Theme {
        match self {
            Theme::Light => Theme::Dark,
            Theme::Dark => Theme::Light,
        }
    }
}

pub fn use_theme() -> ThemeSignal {
    use_context::<ThemeSignal>().expect("ThemeSignal not provided — is this inside <App/>?")
}

/// A saved choice (`localStorage`, from an earlier toggle click) wins;
/// otherwise falls back to the OS/browser's own light-dark preference —
/// the same two-tier resolution `styles/app.css`'s CSS-only fallback
/// uses for a visitor whose Rust hasn't run yet, kept here so the
/// toggle button's icon matches what's actually on screen from the
/// first frame rather than jumping once this runs.
pub fn initial_theme() -> Theme {
    if let Some(saved) = window()
        .local_storage()
        .ok()
        .flatten()
        .and_then(|storage| storage.get_item("theme").ok().flatten())
    {
        return if saved == "dark" { Theme::Dark } else { Theme::Light };
    }
    let prefers_dark = window()
        .match_media("(prefers-color-scheme: dark)")
        .ok()
        .flatten()
        .map(|m| m.matches())
        .unwrap_or(false);
    if prefers_dark { Theme::Dark } else { Theme::Light }
}

/// Sets `<html data-theme="...">` (what every CSS rule keys off) and
/// remembers the choice for next visit. Called from an `Effect` in
/// `app.rs` on mount and on every change to `ThemeSignal`.
pub fn apply_theme(theme: Theme) {
    if let Some(el) = document().document_element() {
        let _ = el.set_attribute("data-theme", theme.as_attr());
    }
    if let Some(storage) = window().local_storage().ok().flatten() {
        let _ = storage.set_item("theme", theme.as_attr());
    }
}
