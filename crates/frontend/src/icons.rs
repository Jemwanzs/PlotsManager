//! Nav icons — small inline SVGs instead of emoji glyphs. Emoji render as
//! flat, differently-styled colour images baked into whatever font the OS
//! ships (visibly out of place next to the app's own typography — the
//! app's owner flagged exactly this, pointing at how a clean, monochrome
//! icon set blends into a sidebar instead of sitting on top of it like a
//! sticker). These use `stroke="currentColor"` and no fill, so every icon
//! automatically follows its surrounding text colour — muted, active,
//! hovered — through the same CSS that already colours the nav label
//! next to it, with no separate icon-colour styling anywhere.

use leptos::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconName {
    Home,
    Projects,
    Customers,
    Quotes,
    Approvals,
    Reports,
    Import,
    Settings,
    Platform,
    Finance,
    Chevron,
    More,
    Sun,
    Moon,
    Eye,
    EyeOff,
}

/// A 20x20 outline icon. Every path below is hand-drawn at the same
/// stroke width/linecap so the set reads as one family instead of icons
/// borrowed from different places.
#[component]
pub fn Icon(name: IconName) -> impl IntoView {
    let body = match name {
        IconName::Home => view! {
            <path d="M3 10.5 12 3l9 7.5" />
            <path d="M5 9.5V20a1 1 0 0 0 1 1h4v-6h4v6h4a1 1 0 0 0 1-1V9.5" />
        }.into_any(),
        IconName::Projects => view! {
            <polygon points="3,6 9,3.5 15,6 21,3.5 21,18 15,20.5 9,18 3,20.5" />
            <line x1="9" y1="3.5" x2="9" y2="18" />
            <line x1="15" y1="6" x2="15" y2="20.5" />
        }.into_any(),
        IconName::Customers => view! {
            <circle cx="12" cy="8" r="3.5" />
            <path d="M5 20.5a7 7 0 0 1 14 0" />
        }.into_any(),
        IconName::Quotes => view! {
            <path d="M7 3h7l5 5v13a1 1 0 0 1-1 1H7a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1Z" />
            <path d="M14 3v5h5" />
            <line x1="8.5" y1="13" x2="15.5" y2="13" />
            <line x1="8.5" y1="16.5" x2="15.5" y2="16.5" />
        }.into_any(),
        IconName::Approvals => view! {
            <circle cx="12" cy="12" r="9" />
            <path d="M8 12.5 10.8 15.3 16 9.5" />
        }.into_any(),
        IconName::Reports => view! {
            <line x1="5" y1="20" x2="5" y2="14" />
            <line x1="12" y1="20" x2="12" y2="6" />
            <line x1="19" y1="20" x2="19" y2="10" />
        }.into_any(),
        IconName::Import => view! {
            <path d="M4 15v4a1 1 0 0 0 1 1h14a1 1 0 0 0 1-1v-4" />
            <path d="M8 9 12 4.5 16 9" />
            <line x1="12" y1="4.5" x2="12" y2="15.5" />
        }.into_any(),
        IconName::Settings => view! {
            <circle cx="12" cy="12" r="3.2" />
            <path d="M12 2.5v3M12 18.5v3M4.2 4.2l2.1 2.1M17.7 17.7l2.1 2.1M2.5 12h3M18.5 12h3M4.2 19.8l2.1-2.1M17.7 6.3l2.1-2.1" />
        }.into_any(),
        IconName::Platform => view! {
            <path d="M12 2.5 19.5 6v6c0 5-3.2 7.8-7.5 9.5-4.3-1.7-7.5-4.5-7.5-9.5V6Z" />
        }.into_any(),
        IconName::Finance => view! {
            <rect x="3" y="6" width="18" height="13" rx="2" />
            <path d="M3 10.5h18" />
            <circle cx="16.5" cy="14.5" r="1.1" fill="currentColor" stroke="none" />
        }.into_any(),
        IconName::Chevron => view! {
            <polyline points="9,6 15,12 9,18" />
        }.into_any(),
        IconName::Sun => view! {
            <circle cx="12" cy="12" r="4.2" />
            <path d="M12 2.5v2.6M12 18.9v2.6M4.6 4.6l1.8 1.8M17.6 17.6l1.8 1.8M2.5 12h2.6M18.9 12h2.6M4.6 19.4l1.8-1.8M17.6 6.4l1.8-1.8" />
        }.into_any(),
        IconName::Moon => view! {
            <path d="M20 14.5A8.5 8.5 0 1 1 9.5 4a6.8 6.8 0 0 0 10.5 10.5Z" />
        }.into_any(),
        IconName::More => view! {
            <circle cx="5" cy="12" r="1.4" />
            <circle cx="12" cy="12" r="1.4" />
            <circle cx="19" cy="12" r="1.4" />
        }.into_any(),
        IconName::Eye => view! {
            <path d="M2 12C4.5 7 8 4.5 12 4.5S19.5 7 22 12c-2.5 5-6 7.5-10 7.5S4.5 17 2 12Z" />
            <circle cx="12" cy="12" r="3" />
        }.into_any(),
        IconName::EyeOff => view! {
            <path d="M2 12C4.5 7 8 4.5 12 4.5S19.5 7 22 12c-2.5 5-6 7.5-10 7.5S4.5 17 2 12Z" />
            <circle cx="12" cy="12" r="3" />
            <line x1="3" y1="3" x2="21" y2="21" />
        }.into_any(),
    };

    view! {
        <svg
            attr:viewBox="0 0 24 24"
            width="20"
            height="20"
            fill="none"
            stroke="currentColor"
            stroke-width="1.75"
            stroke-linecap="round"
            stroke-linejoin="round"
        >
            {body}
        </svg>
    }
}
