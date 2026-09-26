-- Settings-driven, per-category integration configuration — the
-- infrastructure every actual provider integration (SMS, email,
-- WhatsApp, mobile-money/banking payments, accounting) will plug into
-- later, built ahead of any of those providers themselves (2026-09-26
-- decision, docs/14-development-roadmap.md). One row per
-- (organization, category): picking a different provider for a
-- category overwrites its row rather than accumulating several —
-- v1 doesn't need "configured but inactive alternates," just "what's
-- live right now for this category."
--
-- Credentials (`api_key`/`api_secret`/`extra_credentials`) are
-- write-only from the API's perspective — `routes/integrations.rs`
-- never returns their values back out, only whether each is set
-- (`has_api_key`/`has_api_secret`), the same convention this app
-- already uses for user passwords. `config` is free-form JSON for
-- whatever non-secret fields a given provider needs (sender ID, from
-- address, webhook URL, ...) — deliberately not a fixed column set,
-- since every provider's shape differs and hardcoding one would defeat
-- the point of building this generically.
create table integration_configs (
    id uuid primary key default gen_random_uuid(),
    organization_id uuid not null references organizations(id),
    category text not null check (category in (
        'sms', 'email', 'whatsapp', 'payment', 'banking', 'accounting'
    )),
    provider text not null,
    enabled boolean not null default false,
    config jsonb not null default '{}'::jsonb,
    api_key text,
    api_secret text,
    extra_credentials jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    updated_by uuid references users(id),
    unique (organization_id, category)
);
create index integration_configs_org_idx on integration_configs(organization_id);

alter table integration_configs enable row level security;
create policy integration_configs_org_select on integration_configs
    for select using (organization_id = public.current_org_id());
create policy integration_configs_org_insert on integration_configs
    for insert with check (organization_id = public.current_org_id());
create policy integration_configs_org_update on integration_configs
    for update using (organization_id = public.current_org_id());
create policy integration_configs_org_delete on integration_configs
    for delete using (organization_id = public.current_org_id());

update roles set permissions = permissions || '["settings:manage_integrations"]'::jsonb
where not (permissions @> '["*"]'::jsonb) and not (permissions @> '["settings:manage_integrations"]'::jsonb);
