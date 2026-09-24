-- Legacy data migration readiness (Prime Plots gap analysis, continued):
-- title/ownership tracking as structured data rather than the single
-- free-text `plots.title_number` this schema has had since 0001_init —
-- that column stays (still shown as "current title on record" on the
-- plot), but it can't represent a title's own lifecycle: a mother title
-- before subdivision becoming an individual title, a transfer in
-- progress, or a chain of registered owners over time. `title_records`
-- is a plot's title *history* — one row per known title state, newest
-- first — added rather than replacing `plots.title_number` so nothing
-- that already reads that column needs to change.
--
-- No backfill from `plots.title_number` here, deliberately: the
-- gap-analysis instruction was "never guess" on ambiguous legacy data,
-- and a bare title number string gives no registered owner, no issue
-- date, nothing to populate the rest of a row with. Title history
-- starts getting recorded from here forward (including during the
-- actual Prime Plots migration run, a later phase).
create table title_records (
    id uuid primary key default gen_random_uuid(),
    organization_id uuid not null references organizations(id),
    plot_id uuid not null references plots(id),
    title_number text not null,
    registered_owner_name text not null,
    previous_owner_name text,
    title_status text not null default 'individual_title'
        check (title_status in ('mother_title', 'individual_title', 'pending_registration', 'disputed', 'cancelled')),
    transfer_status text not null default 'not_started'
        check (transfer_status in ('not_started', 'in_progress', 'completed')),
    issue_date date,
    registration_date date,
    transfer_date date,
    notes text,
    created_by uuid not null references users(id),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create index title_records_plot_idx on title_records (plot_id, created_at desc);

alter table title_records enable row level security;

create policy title_records_org_select on title_records for select
    using (organization_id = public.current_org_id());
create policy title_records_org_insert on title_records for insert
    with check (organization_id = public.current_org_id());
create policy title_records_org_update on title_records for update
    using (organization_id = public.current_org_id());

-- The document vault (0027_documents.sql) gains a seventh attachable
-- entity: a title record can carry its own scanned title deed / survey
-- plan / consent, distinct from whatever's attached to the plot itself
-- (e.g. a mother title's survey plan vs. an individual title's deed).
alter table documents drop constraint documents_entity_type_check;
alter table documents add constraint documents_entity_type_check
    check (entity_type in ('customer', 'plot', 'project', 'sale', 'loan_account', 'payment', 'title_record'));

-- Backfills the new titles:manage permission onto every existing role —
-- same safety net every permission-enforcement migration this session
-- has used. Recording/updating title history didn't exist as a route
-- before this, so nothing behavioural changes for anyone yet.
update roles
set permissions = permissions || '["titles:manage"]'::jsonb
where not (permissions @> '["*"]'::jsonb)
  and not (permissions @> '["titles:manage"]'::jsonb);
