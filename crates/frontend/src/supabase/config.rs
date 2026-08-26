/// Supabase project URL and anon (public) key, baked in at build time.
///
/// The anon key is meant to be public — same as the Supabase JS client —
/// access control is Postgres Row-Level Security (`supabase/migrations/`),
/// not secrecy of this key. Set both as real environment variables before
/// `trunk build`/`trunk serve` (e.g. in Vercel's project settings, or a
/// local `.env` sourced by your shell — Trunk itself does not read
/// `.env` files).
#[derive(Debug, Clone, Copy)]
pub struct Config {
    pub url: &'static str,
    pub anon_key: &'static str,
}

impl Config {
    pub fn from_env() -> Self {
        Config {
            url: option_env!("SUPABASE_URL")
                .expect("SUPABASE_URL must be set at build time"),
            anon_key: option_env!("SUPABASE_ANON_KEY")
                .expect("SUPABASE_ANON_KEY must be set at build time"),
        }
    }

    pub fn auth_url(&self, path: &str) -> String {
        format!("{}/auth/v1/{path}", self.url)
    }

    pub fn rest_url(&self, path: &str) -> String {
        format!("{}/rest/v1/{path}", self.url)
    }
}
