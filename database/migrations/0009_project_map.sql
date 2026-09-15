-- Minimal interactive-map v1: one image plus one polygon set per
-- project, no draft/pending-approval workflow — re-uploading the
-- image replaces the map outright. docs/06-interactive-map-engine.md
-- Phase 3 specifies a much larger engine (versioned draft->pending->
-- approved->published lifecycle, polygon split/merge, side-by-side
-- comparison); that's deliberately out of scope for this slice.
--
-- Replaces 0001_init.sql's project_map_versions table, which modeled
-- that larger versioning workflow but was never read or written by
-- any route or page (confirmed before writing this migration) — a
-- real image+polygon map beats an unused versioning skeleton, and
-- nothing depends on the old table (no FKs into it, no seed data).
drop table if exists project_map_versions;

create table project_maps (
    project_id uuid primary key references projects(id),
    organization_id uuid not null references organizations(id),
    image_data bytea not null,
    image_content_type text not null,
    -- {"image_width": .., "image_height": .., "features": [{"id":..,
    -- "plot_id":.., "points":[[x,y],...]}]} — pixel coordinates
    -- against the uploaded image, not geographic ones (docs/06's v1
    -- tech choice: plain image + SVG/GeoJSON-shaped overlay, no
    -- GIS/satellite yet). See domain::MapPolygons's module docs.
    polygons jsonb not null default '{"image_width": 0, "image_height": 0, "features": []}',
    uploaded_by uuid not null references users(id),
    updated_at timestamptz not null default now()
);
create index project_maps_organization_id_idx on project_maps(organization_id);

alter table project_maps enable row level security;
create policy project_maps_org_select on project_maps for select
    using (organization_id = public.current_org_id());
