-- Quotations: a formal, time-bound price offer for a plot to a
-- customer, sitting between a lead being interested and a committed
-- sale. Like leads/prospects (0006_lead_pipeline.sql), "quotations and
-- offer letters" was only ever a funnel-stage name in docs/07 — zero
-- fields/lifecycle specified anywhere, and no precedent in the legacy
-- VBA system (docs/02) — this schema is a from-scratch design.
--
-- Deliberately NOT wired into docs/09's approval-workflow engine, which
-- isn't implemented in schema yet (no approval_workflow/approval_step
-- tables exist anywhere). A quote below the plot's minimum_price is
-- surfaced to the caller as computed information
-- (crates/backend/src/routes/quotations.rs's `below_minimum_price`),
-- not enforced as a blocking gate — that gate belongs to the approval
-- engine once it exists, not duplicated ahead of it here.
create table quotations (
    id uuid primary key default gen_random_uuid(),
    organization_id uuid not null references organizations(id),
    plot_id uuid not null references plots(id),
    customer_id uuid not null references customers(id),
    agent_id uuid references users(id),
    payment_mode text not null
        check (payment_mode in ('full_cash', 'lipa_pole_pole_interest_free', 'lipa_pole_pole_interest_bearing')),
    quoted_price numeric(16, 2) not null,
    valid_until date not null,
    -- 'expired' is deliberately not a stored value — same reasoning as
    -- LeadStage in 0006_lead_pipeline.sql: whether a 'sent' quotation
    -- has expired is derived from valid_until at read time, so it can't
    -- go stale without a background job to keep it in sync.
    status text not null default 'draft' check (status in ('draft', 'sent', 'accepted', 'rejected')),
    notes text,
    converted_sale_id uuid references plot_sales(id),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);
create index quotations_organization_id_idx on quotations(organization_id);
create index quotations_customer_id_idx on quotations(customer_id);

alter table quotations enable row level security;
create policy quotations_org_select on quotations for select
    using (organization_id = public.current_org_id());
