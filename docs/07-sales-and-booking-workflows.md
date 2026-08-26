# 07 — Sales and Booking Workflows

## Lifecycle

A plot moves through the status model defined in
[05](05-project-and-plot-management.md#plot-statuses-and-colour-coding):
Available → Selected → Temporarily Held → Reserved → Booked → Under
Approval → Sold → Transfer in Progress → Transferred (with Blocked /
Disputed / Cancelled as side states). Each transition is a discrete,
permission-checked, audited action — not a free-text status edit.

## Sale payment modes

Every plot sale has exactly one approved payment mode
(`domain::PaymentMode`):

1. **Full Cash Sale** — full price paid at once or in closely-spaced
   payments within a configured cash-sale period.
2. **Lipa Pole Pole — Interest-Free** — deposit + scheduled instalments,
   no interest.
3. **Lipa Pole Pole — Interest-Bearing** — deposit + scheduled instalments
   with configurable interest (flat or reducing balance).

Full detail on instalment financing, the Plot Loan Account, and repayment
schedules is in [08](08-payments-and-receipting.md).

## Sales funnel (Phase 5 scope)

Leads and prospects → plot selection → holds/reservations/bookings →
quotations and offer letters → sales agreements → customer 360° view →
agent assignment and commissions.

## Pricing and discount controls

- Both the plot's **published price** and the **final approved selling
  price** are preserved — never overwritten in place.
- Project-level and plot-level prices; separate cash vs. instalment
  pricing.
- Minimum acceptable price and maximum agent discount are configured per
  organisation/project.
- Selling below the configured minimum, or discounting past an agent's
  authority, must trigger the appropriate approval chain
  ([09](09-approval-workflows.md)) rather than being silently allowed.
- Special campaign pricing with start/expiry dates; full price-change
  version history retained.
- Agents only ever see prices and discount options their role is
  authorised for.

## Key invariants

- Every sale links to exactly one customer, project, and plot.
- A plot cannot have more than one *active* sale (joint ownership is an
  explicit, separate configuration, not an accidental double-sale).
- Reservation, hold, and booking rules (expiry, extension, substitution,
  cancellation) are organisation-configurable, not hardcoded.
