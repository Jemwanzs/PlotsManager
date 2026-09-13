use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_navigate;

use crate::auth::use_auth;

struct NavItem {
    href: &'static str,
    icon: &'static str,
    label: &'static str,
}

const NAV_ITEMS: &[NavItem] = &[
    NavItem { href: "/", icon: "\u{1F4CA}", label: "Dashboard" },
    NavItem { href: "/projects", icon: "\u{1F3D8}\u{FE0F}", label: "Projects" },
];

/// Authenticated app layout: sidebar on laptop/desktop, top bar + bottom
/// tab bar on phone/tablet. Route protection lives here too — every
/// protected page's route `view` wraps its content in `<AppShell>`
/// (see `app.rs`), so the "logged in?" check and redirect happen once,
/// not per page.
#[component]
pub fn AppShell(children: Children) -> impl IntoView {
    let auth = use_auth();
    let navigate = use_navigate();

    Effect::new(move |_| {
        if auth.get().is_none() {
            navigate("/login", Default::default());
        }
    });

    let initials = move || {
        auth.get()
            .map(|s| {
                s.user
                    .full_name
                    .split_whitespace()
                    .filter_map(|w| w.chars().next())
                    .take(2)
                    .collect::<String>()
                    .to_uppercase()
            })
            .unwrap_or_default()
    };
    let full_name = move || auth.get().map(|s| s.user.full_name).unwrap_or_default();

    // No <Show when=is_authenticated> around this: the Effect above already
    // redirects to /login the moment `auth` is None, and gating the whole
    // subtree on a signal here would require `children` to be `Fn` (called
    // on every re-render) rather than `FnOnce` (called once at setup),
    // which the mock-auth phase doesn't need — this is UI-layer routing,
    // not the real access control (that's the backend's job, see
    // docs/10-database-and-security-design.md).
    view! {
        <div class="app-shell">
            <aside class="sidebar">
                <div class="sidebar-brand">
                    <span class="logo-mark">"R"</span>
                    <span>"Real Estate Manager"</span>
                </div>
                <nav class="sidebar-nav">
                    {NAV_ITEMS
                        .iter()
                        .map(|item| {
                            view! {
                                <A href=item.href exact=item.href == "/">
                                    <span>{item.icon}</span>
                                    <span>{item.label}</span>
                                </A>
                            }
                        })
                        .collect_view()}
                </nav>
            </aside>

            <header class="topbar">
                <div class="topbar-brand">
                    <span class="logo-mark">"R"</span>
                    <span>"Real Estate Manager"</span>
                </div>
                <div class="topbar-user">
                    <span>{full_name}</span>
                    <span class="avatar">{initials}</span>
                </div>
            </header>

            <main class="main-content">{children()}</main>

            <nav class="bottom-nav">
                {NAV_ITEMS
                    .iter()
                    .map(|item| {
                        view! {
                            <A href=item.href exact=item.href == "/">
                                <span class="icon">{item.icon}</span>
                                <span>{item.label}</span>
                            </A>
                        }
                    })
                    .collect_view()}
            </nav>
        </div>
    }
}
