-- Plot dimensions (side lengths), separate from and never derived from
-- `size` (acreage) -- a plot is described both ways in practice ("0.125
-- acres" and "50 by 100 ft") and neither should overwrite the other.
-- Nullable: existing plots have no recorded dimensions and stay that way
-- until someone edits them; the application shows "Not specified" rather
-- than a misleading 0 x 0.
--
-- `dimension_unit` is stored per plot (not assumed 'ft' app-wide) so a
-- future org-level unit preference (metres, say) can be threaded through
-- without another migration -- every plot created today is seeded with
-- 'ft', the only unit the UI currently offers.
alter table plots
    add column side_1 numeric(10, 2),
    add column side_2 numeric(10, 2),
    add column dimension_unit text not null default 'ft',
    add constraint plots_side_1_positive check (side_1 is null or side_1 > 0),
    add constraint plots_side_2_positive check (side_2 is null or side_2 > 0);
