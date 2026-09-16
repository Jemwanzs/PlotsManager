-- Organization-level configuration: general settings (date format,
-- timezone -- currency already exists on `organizations`) plus a
-- reusable, per-organization auto-numbering engine.
--
-- `numbering_sequences.entity_type` is free text, not a CHECK-constrained
-- enum: the backend (`crates/backend/src/routes/settings.rs`) validates
-- against the set it currently supports ('plot', 'project'), so a later
-- record/document type (docs' example: other document numbering) can
-- adopt this same table without a schema migration.
--
-- `next_number` is incremented via a plain `update ... set next_number =
-- next_number + 1 returning next_number - 1` inside the same transaction
-- as the record insert it's numbering -- deliberately not a Postgres
-- `sequence` (see 0003_plot_loan_account_sequence.sql): a `sequence`
-- doesn't roll back with its transaction, so a failed insert would still
-- burn a number. A row update does roll back, which is what "numbers
-- must not be silently reused, but gaps from a cancelled attempt are
-- fine" actually wants.
create table numbering_sequences (
    id uuid primary key default gen_random_uuid(),
    organization_id uuid not null references organizations(id) on delete cascade,
    entity_type text not null,
    prefix text not null default '',
    include_year boolean not null default false,
    include_entity_code boolean not null default false,
    padding int not null default 4,
    next_number int not null default 1,
    created_at timestamptz not null default now(),
    unique (organization_id, entity_type)
);

alter table organizations
    add column date_format text not null default 'DD/MM/YYYY',
    add column timezone text not null default 'Africa/Nairobi';

-- Every organization gets a plot/project numbering config the moment
-- this migration runs, so `GET /api/v1/settings` never has to special-
-- case "not configured yet" -- new organizations get the same seed at
-- signup (`crates/backend/src/routes/auth.rs`).
insert into numbering_sequences (organization_id, entity_type, prefix, padding)
select id, 'plot', 'PLT', 4 from organizations;

insert into numbering_sequences (organization_id, entity_type, prefix, padding)
select id, 'project', 'PRJ', 4 from organizations;
