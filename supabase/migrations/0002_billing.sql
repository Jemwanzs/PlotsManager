-- SaaS subscription billing for the platform itself (an organization
-- paying Real Estate Manager) via Paystack. Distinct from the in-app
-- customer plot payments in the core schema (0001_init.sql) — see
-- docs/16-billing-and-subscriptions.md for why these are kept separate.
--
-- All writes here happen through the `services` crate using the
-- service_role key (Paystack webhook processing), never directly from the
-- frontend, so only SELECT policies are needed for organizations to view
-- their own billing state.

create table subscription_plans (
    id uuid primary key default gen_random_uuid(),
    code text not null unique,
    name text not null,
    price numeric(12, 2) not null,
    currency text not null default 'KES',
    billing_interval text not null check (billing_interval in ('monthly', 'annual')),
    paystack_plan_code text not null unique,
    is_active boolean not null default true,
    created_at timestamptz not null default now()
);

create table organization_subscriptions (
    id uuid primary key default gen_random_uuid(),
    organization_id uuid not null references organizations(id),
    plan_id uuid not null references subscription_plans(id),
    paystack_customer_code text,
    paystack_subscription_code text,
    status text not null default 'incomplete'
        check (status in ('incomplete', 'trialing', 'active', 'past_due', 'cancelled', 'expired')),
    current_period_start timestamptz,
    current_period_end timestamptz,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique (organization_id)
);

create table billing_invoices (
    id uuid primary key default gen_random_uuid(),
    organization_subscription_id uuid not null references organization_subscriptions(id),
    paystack_reference text not null unique,
    amount numeric(12, 2) not null,
    currency text not null default 'KES',
    status text not null default 'pending' check (status in ('pending', 'paid', 'failed')),
    paid_at timestamptz,
    created_at timestamptz not null default now()
);

-- Idempotency guard: Paystack retries webhooks that don't 200 promptly, so
-- every event id is recorded before it's acted on. Not exposed via the
-- frontend API at all (RLS enabled, zero policies -> default deny; only
-- the service_role connection, which bypasses RLS, can touch it).
create table billing_webhook_events (
    id uuid primary key default gen_random_uuid(),
    provider text not null default 'paystack',
    paystack_event_id text not null,
    event_type text not null,
    payload jsonb not null,
    processed_at timestamptz,
    created_at timestamptz not null default now(),
    unique (provider, paystack_event_id)
);

alter table subscription_plans enable row level security;
alter table organization_subscriptions enable row level security;
alter table billing_invoices enable row level security;
alter table billing_webhook_events enable row level security;

create policy subscription_plans_public_select on subscription_plans for select
    using (is_active = true);

create policy organization_subscriptions_org_select on organization_subscriptions for select
    using (organization_id = public.current_organization_id());

create policy billing_invoices_org_select on billing_invoices for select
    using (organization_subscription_id in (
        select id from organization_subscriptions
        where organization_id = public.current_organization_id()
    ));

-- billing_webhook_events: no policies -> RLS defaults to deny for every
-- role except service_role, which bypasses RLS entirely.
