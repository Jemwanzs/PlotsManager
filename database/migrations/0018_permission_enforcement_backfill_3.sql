-- Backfills the three Reports permissions onto every existing role —
-- same safety net 0016/0017 applied to their own batches. Before this
-- migration, sales_report/inventory_report/agent_performance_report
-- had no require_permission check at all.
--
-- Reports gets this treatment (unlike the CRUD modules, where only
-- the mutating routes were gated and every list/get view stayed open)
-- because a report is itself the sensitive artifact, not a supporting
-- detail view — agent_performance_report in particular exposes
-- individual agent rankings a branch manager might legitimately be
-- allowed to see sales/inventory without also seeing.
--
-- A role with "*" already covers everything and is left alone.
do $$
declare
    key text;
begin
    foreach key in array array[
        'reports:sales',
        'reports:inventory',
        'reports:agent_performance'
    ]
    loop
        update roles
        set permissions = permissions || to_jsonb(array[key])
        where not (permissions @> '["*"]'::jsonb)
          and not (permissions @> to_jsonb(array[key]));
    end loop;
end $$;
