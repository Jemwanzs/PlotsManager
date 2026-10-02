-- Real draft -> published versioning for project maps (docs/06's
-- "every edit creates a new draft version; the currently approved
-- version stays locked and in force until a new one is approved") —
-- the specific gap 0009_project_map.sql's own module comment flagged
-- as a deliberate v1 cut, not an oversight. Replaces the single
-- mutable `project_maps` row per project with real version rows:
-- exactly one `draft` (being edited, invisible to anyone else) and at
-- most one `published` (what everyone not actively editing sees) at a
-- time, plus any number of `superseded` rows kept as read-only
-- history.
create table project_map_versions (
    id uuid primary key default gen_random_uuid(),
    project_id uuid not null references projects(id),
    organization_id uuid not null references organizations(id),
    version_number int not null,
    status text not null check (status in ('draft', 'published', 'superseded')),
    image_data bytea not null,
    image_content_type text not null,
    -- Same shape domain::MapPolygons has always been — pixel
    -- coordinates against this version's own image, not geographic.
    polygons jsonb not null default '{"image_width": 0, "image_height": 0, "features": []}',
    created_by uuid not null references users(id),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    published_by uuid references users(id),
    published_at timestamptz,
    unique (project_id, version_number)
);
create index project_map_versions_project_idx on project_map_versions(project_id, version_number desc);
-- At most one draft and one published version per project at a time —
-- the two states the rest of the design (and the UI's "edit the
-- draft, view the published one" split) relies on being unambiguous.
create unique index project_map_versions_one_draft_per_project
    on project_map_versions(project_id) where status = 'draft';
create unique index project_map_versions_one_published_per_project
    on project_map_versions(project_id) where status = 'published';

alter table project_map_versions enable row level security;
create policy project_map_versions_org_select on project_map_versions for select
    using (organization_id = public.current_org_id());

-- 0037_app_user_role.sql (already applied in production, can't be
-- edited) granted the now-dropped `project_maps` to `app_user` — that
-- grant simply vanishes with the table. This replaces it for the new
-- table, matching that migration's own stated intent of granting
-- ahead of each table's own route file actually switching to
-- `TenantTx` (`routes/project_map.rs` isn't converted in this pass).
grant select, insert, update on project_map_versions to app_user;

-- Every project's existing single map becomes its v1 published
-- version — nothing lost, nothing re-uploaded, no map disappears for
-- an already-mapped project.
insert into project_map_versions
    (project_id, organization_id, version_number, status, image_data, image_content_type,
     polygons, created_by, created_at, updated_at, published_by, published_at)
select project_id, organization_id, 1, 'published', image_data, image_content_type,
       polygons, uploaded_by, updated_at, updated_at, uploaded_by, updated_at
from project_maps;

drop table project_maps;
