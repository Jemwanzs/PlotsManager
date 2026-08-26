# 09 — Approval Workflows

## Model

The platform supports unlimited approval levels ("N-level") for any
sensitive action, configured per organisation rather than hardcoded.

Each workflow supports: any number of steps; approver assignment by user,
position, role, branch, or project; sequential or parallel approvals;
amount- and discount-based conditions; return for amendment; rejection
with reasons; delegation and escalation; reminders and expiry; separation
of duties; email/SMS/WhatsApp/in-app notifications; immutable approval
history.

## Actions gated by approval

Project/plot setup: new project creation, project-plan publication, plot
creation/numbering, price setup/changes, discounts, selling below the
approved minimum price.

Sales/booking: reservations, reservation extensions, plot substitutions,
booking cancellation, customer reassignment, plot blocking/reopening.

Payments/finance: payment reversals, commission approval, sale completion,
interest-rate changes, payment verification/posting, backdated payments,
waivers/write-offs, repayment-plan restructures, grace periods/payment
holidays, early settlement.

Ownership: title-transfer initiation, document replacement, cancellation/
repossession/reallocation, full-settlement confirmation.

## Design notes for implementation

- Approvers may be assigned by user, position, role, branch, project,
  amount threshold, or exception type — the rule engine needs to support
  all of these as conditions on a single workflow definition, not separate
  code paths per action type.
- Every approval decision (approve/reject/return/delegate/escalate) is
  appended to an immutable history, not overwritten — this feeds directly
  into the `audit_log` table (see
  [10](10-database-and-security-design.md#schema)).
- Workflow definitions are organisation data, not code, so a tenant can
  add/remove approval steps without a deployment.

## Legacy reality (see [02](02-existing-vba-system-analysis.md))

The system being replaced has exactly one approval step for payments and
charges: a dropdown (Approved/Reject) with a free-text remark, reachable
by any logged-in user — nothing in the code stops the person who captured
a payment from also approving it. This doc's separation-of-duties and
role-scoped approver assignment aren't a refinement of an existing
control, they're introducing the control for the first time. Treat "does
this action require an approver different from its initiator" as a
setting to get right early, since the legacy system offers no precedent
for how strict that should be — that's a product decision, not something
inferable from history.
