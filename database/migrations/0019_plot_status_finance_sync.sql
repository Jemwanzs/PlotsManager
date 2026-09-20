-- One-time backfill alongside the code fix in
-- crates/backend/src/routes/sales.rs (execute_sale, insert_bulk_sale)
-- and crates/backend/src/routes/loan_accounts.rs (record_payment):
-- plot status was set once at sale time and never updated again, so
-- an already fully-paid plot stayed stuck at "Booked"/"Reserved"
-- forever on every screen that reads plot status. The code fix only
-- changes behaviour going forward; this repairs data that's already
-- wrong today.
--
-- Two cases, matching the two places the code now advances a plot to
-- Sold:
--
-- 1. Every full-cash sale's plot — a cash sale is paid in full the
--    moment it's recorded, so any plot still sitting at its old
--    'reserved' default was never actually incomplete, just never
--    updated.
update plots
set status = 'sold'
where status in ('reserved', 'booked', 'selected', 'temporarily_held', 'under_approval')
  and id in (
      select ps.plot_id
      from plot_sales ps
      where ps.payment_mode = 'full_cash'
  );

-- 2. Every Lipa Pole Pole plot whose loan account has already reached
--    'fully_paid' — same status the plot itself should now show.
update plots
set status = 'sold'
where status in ('reserved', 'booked', 'selected', 'temporarily_held', 'under_approval')
  and id in (
      select ps.plot_id
      from plot_sales ps
      join plot_loan_accounts pla on pla.sale_id = ps.id
      where pla.status = 'fully_paid'
  );
