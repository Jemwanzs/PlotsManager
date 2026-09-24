-- Legacy data migration readiness (Prime Plots gap analysis, final
-- Phase 1 piece): a real staging/migration framework, replacing the
-- closest existing analog — `POST /api/v1/customers/bulk` — which
-- parses a CSV client-side and writes straight to `customers` with no
-- staging, no raw-value preservation, and no exceptions queue. That
-- route stays (it serves a different, narrower purpose: a fast
-- best-effort add during tenant onboarding); this is the more
-- rigorous path for an actual historical data migration.
--
-- `migration_batches` is one uploaded source file; `migration_staging_rows`
-- is one row of it. Nothing lands in a production table (`customers`,
-- eventually `plots`/`sales`/...) until a batch is explicitly committed
-- — see `crates/backend/src/routes/migrations.rs`'s module docs for the
-- full Upload -> Validate -> Resolve Exceptions -> Commit flow this
-- supports today (Customer only; the schema is entity-agnostic so
-- Plot/Project/Sale/LoanAccount follow the same pattern later without
-- another migration).
--
-- `raw_data` is the row exactly as uploaded, untouched, forever —
-- "never guess" from the gap analysis: even a row that fails
-- validation keeps its original values on file rather than being
-- dropped, so a human can see exactly what the source file said.
-- `normalized_data` is the mapped/cleaned version computed from it;
-- editing a row during "resolve exceptions" updates both, re-running
-- the same validation raw_data went through.
create table migration_batches (
    id uuid primary key default gen_random_uuid(),
    organization_id uuid not null references organizations(id),
    entity_type text not null check (entity_type in ('customer')),
    source_system text not null,
    source_file_name text not null,
    status text not null default 'staged' check (status in ('staged', 'committed')),
    total_rows int not null default 0,
    valid_rows int not null default 0,
    exception_rows int not null default 0,
    committed_rows int not null default 0,
    created_by uuid not null references users(id),
    created_at timestamptz not null default now(),
    committed_at timestamptz
);

create table migration_staging_rows (
    id uuid primary key default gen_random_uuid(),
    batch_id uuid not null references migration_batches(id),
    source_row int not null,
    raw_data jsonb not null,
    normalized_data jsonb not null,
    status text not null default 'exception' check (status in ('valid', 'exception')),
    exception_message text,
    -- Set once this row is committed into its target production table
    -- (e.g. customers.id) — lets a re-run of commit skip rows it
    -- already placed rather than creating duplicates.
    committed_entity_id uuid,
    unique (batch_id, source_row)
);
create index migration_staging_rows_batch_idx on migration_staging_rows(batch_id);

-- Traceability onto the target table itself, per the gap analysis's
-- migration_batch_id/legacy_id instruction — legacy_customer_number
-- (0025) already covers the "legacy_id" half for customers.
alter table customers add column migration_batch_id uuid references migration_batches(id);

alter table migration_batches enable row level security;
alter table migration_staging_rows enable row level security;

create policy migration_batches_org_select on migration_batches for select
    using (organization_id = public.current_org_id());
create policy migration_batches_org_insert on migration_batches for insert
    with check (organization_id = public.current_org_id());
create policy migration_batches_org_update on migration_batches for update
    using (organization_id = public.current_org_id());

create policy migration_staging_rows_org_select on migration_staging_rows for select
    using (batch_id in (select id from migration_batches where organization_id = public.current_org_id()));
create policy migration_staging_rows_org_insert on migration_staging_rows for insert
    with check (batch_id in (select id from migration_batches where organization_id = public.current_org_id()));
create policy migration_staging_rows_org_update on migration_staging_rows for update
    using (batch_id in (select id from migration_batches where organization_id = public.current_org_id()));

-- Backfills the new migrations:manage permission onto every existing
-- role — same safety net every permission-enforcement migration this
-- session has used. This entire capability is new, so nothing
-- behavioural changes for anyone yet.
update roles
set permissions = permissions || '["migrations:manage"]'::jsonb
where not (permissions @> '["*"]'::jsonb)
  and not (permissions @> '["migrations:manage"]'::jsonb);
