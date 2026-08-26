# 08 — Payments and Receipting

Covers full settlement from reservation through full payment, for both
cash sales and Lipa Pole Pole (instalment) sales. First release supports
**manual capture only**, by authorised officers — see
[12](12-api-and-integration-design.md) for the future
integration path (mobile money, bank, card) that must reuse this same
engine rather than maintaining a parallel balance.

## Plot Loan Account

Every instalment sale creates a **Plot Loan Account**: a receivable and
repayment account secured against the plot — not a disbursed cash loan.
Modelled as `domain::PlotLoanAccount`
([crates/domain/src/sales.rs](../crates/domain/src/sales.rs)).

Fields: unique account number; customer/co-buyer; org/branch/project/plot;
original listed price and approved selling price; deposit required/paid;
financed principal; interest method/rate if applicable; charges/taxes;
repayment period/frequency; instalment amount; start/expected-completion
dates; amount paid/due/overdue and remaining balance; next instalment date
and amount; days in arrears; status; assigned agent/account officer;
approval, payment, and communication history.

### Statuses (`domain::LoanAccountStatus`)

Draft → Pending Approval → Approved (Awaiting Deposit) → Active/Current ↔
Active/Partially Paid ↔ In Grace Period ↔ In Arrears → (Restructured |
Settlement Pending Verification) → Fully Paid → Closed, with Cancelled /
Defaulted / Repossessed-or-Reallocated as terminal side states.
Transitions are configurable, permission-controlled, and audited.

## Repayment schedule

Generated on approval: instalment number, due date, opening principal,
principal/interest/fees due, total due, amount paid, date paid, remaining
amount, closing balance, status (Upcoming / Due / Partially Paid / Paid /
Overdue). Modelled as `domain::RepaymentScheduleEntry`.

Recalculated (never edited in place — original schedule stays available
for audit/comparison) on: early/excess payment, partial payment, late
payment, approved interest/penalty waiver, price adjustment, payment
reversal, restructure, approved payment holiday, or plot
substitution/cancellation.

## Manual payment capture

Fields: customer; project/plot; loan account (or cash-sale account);
payment/value date; amount; method (cash, bank transfer, cheque, mobile
money, card, other); external reference; receiving bank/collection
account; narration; proof-of-payment attachment; captured-by + device/session;
verification/approval status. Customer/project/plot auto-populate from the
selected account so an officer cannot misallocate to the wrong plot.

### Allocation

Each approved payment is allocated per a configurable priority (suggested
default: fees/charges → penalties/overdue interest → regular interest →
overdue principal → current principal → future principal/credit).
Also supported: one receipt split across multiple plots for the same
customer (with explicit confirmation), unallocated receipts held in
suspense until resolved, overpayments held as customer credit, duplicate-
reference detection, backdated-payment controls, maker-checker
verification.

### Verification lifecycle (`domain::PaymentStatus`)

Captured → Verified → Posted (only Posted payments affect the official
balance) → Rejected (with reason) → Reversed (counter-transaction,
approval-gated). **Posted payments are never edited or deleted** —
corrections are a reversal plus a replacement transaction, full audit
trail preserved.

## Customer account and portfolio

Customer 360° view: all plots purchased/reserved/under repayment with
per-plot account status; totals for purchase value, paid, principal,
interest, charges, penalties, outstanding, overdue; upcoming instalments;
receipts/payment history; statements/documents; sales agreements/approval
records; assigned agent/officer; collection notes; title-transfer
readiness. Every payment appears both in the consolidated view and the
specific plot account it was allocated to.

## Statements and documents

- **Plot account statement** — branded, itemised transaction history,
  opening/closing balance, arrears/next-payment info, verification/QR
  code.
- **Consolidated customer statement** — summary across all plots a
  customer owns, with per-plot statements as appendices.
- Other documents: receipt, payment acknowledgement, instalment schedule,
  balance confirmation, arrears/reminder notice, demand notice,
  early-settlement quotation, full-settlement certificate, plot clearance
  certificate, cancellation/reallocation/repossession notice,
  title-transfer readiness letter.

All documents: PDF export, printable, emailable from the system, retained
in customer document history with recipient/channel/sender/date/delivery
status/version recorded.

## Collections and arrears

Proactive, not just reactive record-keeping: upcoming/due-today reminders,
configurable grace periods, automatic arrears ageing (suggested buckets:
Current, 1–30, 31–60, 61–90, 91–180, 180+ days, all configurable),
days-past-due, risk/delinquency categories, promise-to-pay capture,
collection notes/contact attempts, account-officer work queues,
escalation, and restructure/waiver/cancellation/repossession workflows —
each behind the approval engine ([09](09-approval-workflows.md)).

## Key business rules

- A payment affects the official balance only after it reaches the
  required verification/approval status.
- Interest, charges, and penalties are generated only from approved
  configurations — never ad hoc.
- Fully-paid status is system-calculated, with an authorised settlement
  confirmation where configured.
- Title transfer may begin only once all required financial, legal, and
  approval conditions are satisfied.
- Dashboard totals, statements, plot accounts, and reports must all
  reconcile to the same transaction ledger.

## Delivery sequence

**Phase A** (core): cash + Lipa Pole Pole sale modes, Plot Loan Accounts,
repayment schedules, manual capture/allocation, approvals/reversals,
receipts/statements.
**Phase B**: arrears ageing, notifications/work queues, customer 360°,
dashboards, report library.
**Phase C**: restructures/waivers/holidays, cancellations/repossessions/
reallocations, agent commissions, title-transfer readiness.
**Phase D**: mobile-money/banking integrations, automated matching/
receipting/reconciliation, customer self-service.
