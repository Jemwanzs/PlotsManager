-- 0038_project_map_versions.sql granted insert/update on this table to
-- app_user (ahead of routes/project_map.rs actually converting to
-- TenantTx, matching 0037's own stated intent) but only ever created
-- a select RLS policy. Table-level privilege without a matching RLS
-- policy means every row is denied for that command regardless of
-- the grant: harmless today since this route still runs on the
-- unrestricted connection role, but it would turn a routine future
-- TenantTx conversion into an outage for every map upload/draft/
-- publish the moment it switched over, with no policy-related change
-- visible in that diff to explain why. Adding the matching policies
-- now, before anything depends on them being there.
create policy project_map_versions_org_insert on project_map_versions for insert
    with check (organization_id = public.current_org_id());
create policy project_map_versions_org_update on project_map_versions for update
    using (organization_id = public.current_org_id());
