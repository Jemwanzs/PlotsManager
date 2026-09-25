-- Agent commissions — accrual tracking only (no payout/settlement
-- workflow yet, per explicit product decision): a commission accrues
-- the moment a sale is recorded, on the full agreed price, at whatever
-- rate applies — a project's own `commission_rate_percent` if set,
-- else the organization's `default_commission_rate_percent`. One row
-- per sale (not per plot on a multi-plot sale — `agreed_price` is
-- already transaction-wide, matching the same "one loan account per
-- whole transaction" shape `plot_loan_accounts` already uses).
alter table organizations
    add column default_commission_rate_percent numeric(6, 3) not null default 0;

alter table projects
    add column commission_rate_percent numeric(6, 3);

create table agent_commissions (
    id uuid primary key default gen_random_uuid(),
    organization_id uuid not null references organizations(id),
    sale_id uuid not null references plot_sales(id),
    agent_id uuid not null references users(id),
    rate_percent numeric(6, 3) not null,
    agreed_price numeric(16, 2) not null,
    commission_amount numeric(16, 2) not null,
    -- Set by `routes/sales.rs::cancel_sale`/`repossess_sale` when the
    -- underlying sale is unwound — the commission row stays (an
    -- accurate historical record of what accrued and when), but
    -- reports exclude it from an agent's earned total once voided.
    voided_at timestamptz,
    created_at timestamptz not null default now(),
    unique (sale_id)
);
create index agent_commissions_agent_idx on agent_commissions(agent_id);

alter table agent_commissions enable row level security;
create policy agent_commissions_org_select on agent_commissions for select
    using (organization_id = public.current_org_id());
create policy agent_commissions_org_insert on agent_commissions for insert
    with check (organization_id = public.current_org_id());
create policy agent_commissions_org_update on agent_commissions for update
    using (organization_id = public.current_org_id());

-- No new permission key: viewing commission totals rides the existing
-- `reports:agent_performance` key, whose own registry comment already
-- flags it as "compensation-adjacent" — the exact fit for this.
