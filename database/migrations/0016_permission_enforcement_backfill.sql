-- Backfills three permission keys onto every existing role so that
-- newly-added backend enforcement (crates/backend/src/routes/
-- settings.rs::update_settings, approvals.rs::decide, loan_accounts.rs
-- ::record_payment) never regresses access a role already had.
--
-- Before this migration, these three actions had no require_permission
-- check at all — any authenticated org member could edit org settings,
-- decide a price approval, or record a payment. The permission keys
-- below (settings:manage_organization is new; approvals:approve and
-- payments:record already existed in the catalog but were never
-- actually enforced by any route) now gate those actions. Without this
-- backfill, every pre-existing non-wildcard role would silently lose
-- an ability it had yesterday the moment this deployed.
--
-- A role with "*" already covers everything and is left alone; a role
-- that already happens to list one of these keys (possible for
-- approvals:approve/payments:record, which existed in the catalog
-- before this migration even though nothing checked them) is also
-- left alone rather than duplicated.
update roles
set permissions = permissions || '["settings:manage_organization"]'::jsonb
where not (permissions @> '["*"]'::jsonb)
  and not (permissions @> '["settings:manage_organization"]'::jsonb);

update roles
set permissions = permissions || '["approvals:approve"]'::jsonb
where not (permissions @> '["*"]'::jsonb)
  and not (permissions @> '["approvals:approve"]'::jsonb);

update roles
set permissions = permissions || '["payments:record"]'::jsonb
where not (permissions @> '["*"]'::jsonb)
  and not (permissions @> '["payments:record"]'::jsonb);
