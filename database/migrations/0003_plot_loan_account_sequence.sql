-- Atomic, collision-free account numbering for Plot Loan Accounts.
-- Deliberately not a `count(*) + 1` (or worse, a row-count-based scheme
-- like the legacy system's `PPP_LN00<row>` — docs/02 §3 flags exactly
-- this as a bug: it collides under concurrent inserts and breaks
-- entirely once a row is ever deleted). A sequence's nextval() is
-- allocated outside the calling transaction's lock scope, so two
-- concurrent sales can never be handed the same number.
create sequence plot_loan_account_number_seq;
