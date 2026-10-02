-- Opt-in gate for the Captured -> Verified-and-Posted payment
-- lifecycle docs/08-payments-and-receipting.md always specified
-- (`domain::PaymentStatus` already has every state) but
-- `routes/loan_accounts.rs::record_payment` has always skipped,
-- posting every payment immediately. Off by default — an existing
-- organization's daily workflow doesn't change unless it opts in from
-- Settings. `payments`/`plot_loan_accounts`/`loan_ledger_entries`
-- already have every column this needs; no other schema change.
alter table organizations
    add column require_payment_approval boolean not null default false;

-- Nowhere to record why a captured payment was rejected (status
-- already defaulted to 'captured' and allowed 'posted'/'rejected' as
-- free text from day one, `0001_init.sql` — no check constraint ever
-- enforced the full `domain::PaymentStatus` set, so no constraint
-- change is needed here, just this column `reject_payment` needs).
alter table payments add column rejection_reason text;
