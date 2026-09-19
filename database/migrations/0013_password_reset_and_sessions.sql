-- Password reset (admin-initiated) and session security: a temporary
-- password issued by an admin must be changed before it's trusted for
-- anything else, an expired one is rejected at login, and any reset
-- (admin- or self-initiated) invalidates already-issued session tokens
-- without needing a server-side session table — `session_valid_after`
-- is compared against the JWT's `iat` claim on every authenticated
-- request (crates/backend/src/extractors.rs).

alter table users
    add column password_changed_at timestamptz not null default now(),
    add column must_change_password boolean not null default false,
    add column temp_password_expires_at timestamptz,
    add column session_valid_after timestamptz not null default now();
