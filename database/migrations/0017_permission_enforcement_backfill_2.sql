-- Backfills every permission key newly enforced in this batch onto
-- every existing role, the same safety net 0016 applied to the first
-- three. Before this migration, none of Projects/Plots/Customers/
-- Quotes had any require_permission check on their mutating routes —
-- any authenticated org member could create/edit a project or plot,
-- draw map boundaries, link a plot to a shape, reserve/book a plot,
-- bulk-import plots/customers/sales, update a lead's stage, or
-- create/send/accept/reject a quotation. Without this backfill, every
-- pre-existing non-wildcard role would silently lose all of that the
-- moment this deployed.
--
-- A role with "*" already covers everything and is left alone.
do $$
declare
    key text;
begin
    foreach key in array array[
        'projects:create',
        'plots:create',
        'plots:edit',
        'plots:bulk_import',
        'plots:map.upload',
        'plots:map.edit_boundaries',
        'plots:map.link',
        'plots:transactions.create',
        'plots:transactions.bulk_import',
        'customers:create',
        'customers:bulk_import',
        'customers:leads.update',
        'quotes:create',
        'quotes:send',
        'quotes:reject',
        'quotes:accept'
    ]
    loop
        update roles
        set permissions = permissions || to_jsonb(array[key])
        where not (permissions @> '["*"]'::jsonb)
          and not (permissions @> to_jsonb(array[key]));
    end loop;
end $$;
