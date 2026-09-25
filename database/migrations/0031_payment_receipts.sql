-- Per-payment receipts — distinct from the loan statement PDF (a
-- running account summary across every transaction): a receipt is the
-- one-page proof-of-payment handed to a customer at the moment their
-- payment is recorded. `receipt_number` mirrors `plot_loan_accounts.
-- account_number`'s own numbering (`0003_plot_loan_account_sequence.sql`)
-- — a real Postgres sequence, not a `count(*) + 1` scheme (docs/02 §3
-- flags exactly that as a legacy bug: it collides under concurrent
-- inserts and breaks once a row is ever deleted).
create sequence payment_receipt_number_seq;

alter table payments add column receipt_number text;

-- Backfill every existing payment in the order it actually happened
-- (`created_at`), not insertion order, so receipt numbers reflect real
-- chronology for historical/migrated data too.
with numbered as (
    select id, row_number() over (order by created_at) as rn from payments
)
update payments
set receipt_number = 'RCT-' || lpad(numbered.rn::text, 5, '0')
from numbered
where payments.id = numbered.id;

select setval('payment_receipt_number_seq', (select count(*) from payments), true);

alter table payments alter column receipt_number set not null;
create unique index payments_receipt_number_uidx on payments (receipt_number);
