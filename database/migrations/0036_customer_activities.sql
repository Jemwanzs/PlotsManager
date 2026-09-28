-- A customer's communications log — the "customer 360" gap the
-- roadmap flagged as genuinely missing (docs/14-development-roadmap.md):
-- `customers.notes` is a single current-state field, one note at a
-- time, not a dated history of what actually happened. Append-only,
-- like every other ledger/audit trail in this app (loan_ledger_entries,
-- audit_log) — a mistake gets a corrective follow-up entry, not a
-- silent edit.
create table customer_activities (
    id uuid primary key default gen_random_uuid(),
    organization_id uuid not null references organizations(id),
    customer_id uuid not null references customers(id),
    activity_type text not null check (activity_type in (
        'call', 'email', 'sms', 'whatsapp', 'meeting', 'site_visit', 'note', 'other'
    )),
    summary text not null,
    occurred_at timestamptz not null default now(),
    created_by uuid not null references users(id),
    created_at timestamptz not null default now()
);
create index customer_activities_customer_idx on customer_activities(customer_id, occurred_at desc);
create index customer_activities_org_idx on customer_activities(organization_id);

alter table customer_activities enable row level security;
create policy customer_activities_org_select on customer_activities
    for select using (organization_id = public.current_org_id());
create policy customer_activities_org_insert on customer_activities
    for insert with check (organization_id = public.current_org_id());
