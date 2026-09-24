-- Generic document vault (legacy-migration-readiness spec section 4-6):
-- a single polymorphic table rather than per-entity attachment columns,
-- so "attach a scanned ID / title deed / agreement to X" never requires
-- a schema change for a new entity type or a new document category.
-- Storage is Postgres bytea (explicit choice, matching the existing
-- project_maps.image_data precedent) rather than an external object
-- store — fine at current scale, revisit if/when file volume grows.
--
-- `document_type` is free text (validated by the backend against a
-- known set per entity type) rather than a Postgres enum, so adding a
-- new legacy document category during the actual migration run never
-- needs a migration of its own.
--
-- `migration_batch_id` has no FK yet — the migration_batches table is
-- future work (Phase 2, the staging/reconciliation framework). It's a
-- plain nullable uuid placeholder so documents uploaded/migrated now
-- don't need to be touched again once that table exists.
create table documents (
    id uuid primary key default gen_random_uuid(),
    organization_id uuid not null references organizations(id),
    entity_type text not null check (entity_type in ('customer', 'plot', 'project', 'sale', 'loan_account', 'payment')),
    entity_id uuid not null,
    document_type text not null,
    document_number text,
    original_filename text not null,
    mime_type text not null,
    file_size bigint not null,
    file_data bytea not null,
    issue_date date,
    expiry_date date,
    description text,
    uploaded_by uuid not null references users(id),
    uploaded_at timestamptz not null default now(),
    legacy_source_path text,
    migration_batch_id uuid
);

create index documents_entity_idx on documents (organization_id, entity_type, entity_id);

alter table documents enable row level security;

create policy documents_org_select on documents for select
    using (organization_id = public.current_org_id());
create policy documents_org_insert on documents for insert
    with check (organization_id = public.current_org_id());
create policy documents_org_delete on documents for delete
    using (organization_id = public.current_org_id());

-- Backfills the new documents:manage permission onto every existing
-- role — same safety net every permission-enforcement migration this
-- session has used. Upload/delete didn't exist as routes before this,
-- so nothing behavioural changes for anyone yet.
update roles
set permissions = permissions || '["documents:manage"]'::jsonb
where not (permissions @> '["*"]'::jsonb)
  and not (permissions @> '["documents:manage"]'::jsonb);
