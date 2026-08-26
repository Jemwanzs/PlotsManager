# 02 — Existing VBA System Analysis

**Status: complete** (first pass). Source: `legacy-excel/PlotsManager/PPP_v.01.Xls.xlsm`
("Prime Plots Property & Consultants" — Operations Management System),
VBA extracted programmatically with `oletools` (58 modules: 22 UserForms,
3 standard modules, `ThisWorkbook`, and ~31 auto-generated, empty
`Sheet*.cls` stubs) into `legacy-excel/exported-vba/` (gitignored, local
only — see [13](13-data-migration-plan.md) on why nothing from it is
copied verbatim into tracked files). This document is prose analysis
only; no legacy code or real data is reproduced here.

This is the functional spec of record for what the business actually does
today. Where it conflicts with earlier inference in docs 03–11, **this
document wins** — those docs have been updated with "Legacy reality" notes
pointing back here.

## 1. Shape of the system

One Excel workbook, opened by one person at a time, with ~22 VBA
UserForms as the entire UI (no worksheet is meant to be edited directly).
Every form maximizes the Excel window and hides the ribbon to look like a
standalone app. All data lives in worksheets acting as tables; VBA reads
and writes them with `CountA`-based "next row" logic (no real primary
keys, no referential integrity — enforced only by ad hoc `CountIf`
duplicate checks scattered per form).

Business identity found in the workbook: **Prime Plots Property &
Consultants** ("Beyond the Sale"), operating under project codes like
`KJD-KIT` and `KCA`. Not reproduced here beyond what's needed for context
— see the workbook itself for details, and do not put real branding into
tracked docs without the user's say-so.

## 2. Data model (worksheets = tables)

| Sheet | Role | Key columns observed |
|---|---|---|
| `SetUps` | **User accounts.** Max 5 users (hard capped in code). B=username, C=password (plaintext), D=session flag ("Activated"/blank). | No roles beyond a hardcoded `"Admin"` string check. |
| `Projects` | Land projects | B=Name (+`_Phase` suffix if a phase is chosen, e.g. `Riverside_Phase II`), C=County, D=Location, E/F=created at/by |
| `Project_PLOTS` | Plot register | B=**Plot Description** (free text — this is the plot's actual identity/key, not a separate number), C=Project, D=Location (VLOOKUP from Projects), E=Size, F=Price, G/H=created at/by |
| `CustomerInfo` | Customers + KYC | B=Customer No (`PPP_C00<row>`), C=Title, D=Name, E=National ID/Passport, F=Postal, G=City, H=Mobile, I=Email, J=KRA PIN, K=Join date, L/M/N=file paths to uploaded face/ID/KRA photos (copied into `#images/`), O–T=next-of-kin (name, relationship, 2 mobiles, ID, city) |
| `LoanRegister` | One row per plot sale (**every sale is a "loan" — see §4**) | B=Plot Description, C=Project, D=Loan No (`PPP_LN00<row>`), E=Customer No, G=Loan date, I=Purchase value, J=Tenor, K=EMI (instalment amount), L=Expected deposit, M=Due date (**free text**, e.g. "30th monthly" — not a real date), S=Status (`Active` / `Closed Loan Account`, set manually), T/V=last-approved payment date/amount |
| `LoanPayment` | Pending payment entries awaiting approval | Plot, Loan No, Project, balance snapshot, pay date, receipt ref, amount, description, new balance, channel |
| `OtherCharges` | Pending interest/penalty charges awaiting approval | Same shape as `LoanPayment`, charge type instead of payment description |
| `PrintLoan` | **The one ledger.** Every *approved* payment, charge, and overpayment debit lands here — this single sheet drives the customer statement PDF, `frm_REPORTING`, and `frm_MONITORING`. | Same columns as above + approval remark |
| `AllRejected` | Rejected payments/charges | Same shape, for audit only |
| `SuspenseAccount` | Company-wide clearing account | Interest/penalty payments and overpayment debits post here; a single running balance lives in `Support!B16`, decremented directly by code — **not per-customer** |
| `LnOverpaymentRfd` | Overpayment debits (see §6) | Reason, amount, narration |
| `Comments` / `Cmt_Display` | Customer feedback | Type (Rating/Question/Complain/Others), text, gender, 1–5 rating |
| `DailyReport` / `DailyReportBackUp` / `StaffReport` | Staff daily activity KPIs (see §7) | Site visits, savings, sales, new leads, office visits, walk-ins, instalments, transfers |
| `S_Activities` | Navigation/session log (**not** a data-change audit log) | Screen name, timestamp, Start/Ended, username |
| `Support` | Precomputed dashboard KPIs (Excel formulas, not VBA) | Totals referenced by `frm_MAIN`'s dashboard |
| `D_*` sheets (`D_Project_PLOTS`, `D_LoanRegister`, `D_CustomerInfo`, `D_Projects`, `D_PrintLoan`, `D_StaffReport`) | Scratch copies used to drive AutoFilter-based search/list views | Rebuilt on every search keystroke |

## 3. Numbering

- **Projects**: `Name` or `Name_Phase` (e.g. `Phase I`…`Phase X` from a
  fixed dropdown) — a string, not a code.
- **Plots**: no separate plot number at all. The **Plot Description**
  free-text field *is* the identity, and it is checked for uniqueness
  **globally across the whole workbook**, not per project — two different
  projects cannot use the same plot description.
- **Loans**: `PPP_LN00<row-count>` — derived from the current row count of
  `LoanRegister`, not a stored sequence. Breaks on deletion, and has no
  digit padding (`PPP_LN0010`, `PPP_LN00100`, …).
- **Customers**: `PPP_C00<row-count>` — same pattern and same weakness.

**Planned improvement**: [05](05-project-and-plot-management.md) and
[10](10-database-and-security-design.md) already specify system-generated
UUIDs plus a separate, tenant-configurable human-readable sequence — this
legacy behavior is exactly the failure mode that design avoids.

## 4. There is no "Full Cash Sale" — only Lipa Pole Pole

Every plot sale, cash or instalment, is entered through `frm_LOAN_Reg` as
a "loan": purchase value, tenor, EMI, expected deposit, due date — all
**typed in by the officer**, not calculated. There is:

- **No interest rate field, no compounding, no amortization.** Interest
  and penalties are separate manual "charges" applied later through
  `frm_Finance`'s charge tab (`INTEREST DUE` / `PENALTY DUE`), which
  simply add a flat, human-entered amount to the balance.
- **No generated repayment schedule.** The one PDF that looks like a
  schedule (`frm_CUSTOMER_DEBT_SCHEDULE` → `#PaymentSchedulesPDF/`) is a
  **separately, manually re-typed** summary (customer + plot + price +
  deposit + tenor + EMI + contractual date) — not derived from
  `LoanRegister`, and not a per-instalment due-date table.
- **A resale override** (`cbRESALE` = "Resale_1/2/3") lets an officer
  bypass the "plot already sold" duplicate check to re-register a plot
  that was repossessed/cancelled — there is no real cancellation/
  repossession status; the plot simply gets a second `LoanRegister` row.

**Planned improvement**: this is precisely the gap [08](08-payments-and-receipting.md)'s
Plot Loan Account + generated, recalculable repayment schedule is designed
to close, and it should be treated as a genuine upgrade, not a
reimplementation — there is no legacy interest engine to match, only a
manual-charge habit to preserve as a fallback ("manual adjustment"
capability) for edge cases the automatic engine won't cover.

## 5. Payments, charges, and the single-step "approval"

`frm_Finance` captures a payment (description: `INSTALMENT` / `DEPOSIT
PAYMENT` / `RESERVATION` / `INTEREST PAYMENT` / `PENALTY PAYMENT`;
channel: `M-PESA TILL` / `KCB BANK` / `FAMILY BANK` / `CASH` / `SUSPENSE
ACCOUNT`) into `LoanPayment`, a holding sheet. `INTEREST PAYMENT` and
`PENALTY PAYMENT` are force-routed to the `SUSPENSE ACCOUNT` channel
(dropdown locked) rather than reducing principal directly.

**Approval** (`frm_Approvals` / `FRM_ApproveCharges`) is a single dropdown
(`APPROVED` / `REJECT`) with a free-text remark, reachable by **any**
logged-in user with no check that the approver differs from the person
who captured the payment — there is no maker-checker separation in code,
only in intended practice. Approved rows move to `PrintLoan` (posted
ledger); rejected rows move to `AllRejected`.

**Reversal** re-opens the payment form pre-filled from the original row
with the amount negated and every field locked except the reference
(suffixed `;Reversal`) — a genuinely good pattern worth keeping: a
reversal is generated *from* the original transaction, not freehand. One
explicit guard exists: a reversal of a reversal is blocked.

**Planned improvement**: [08](08-payments-and-receipting.md)'s
Captured → Verified → Posted lifecycle and [09](09-approval-workflows.md)'s
N-level, role-scoped approval chain directly replace this. Keep the
"reversal pre-fills and locks from the original" UX pattern — it's the one
piece of this flow worth carrying forward as-is.

## 6. Suspense account and overpayment handling

`SuspenseAccount` is a **single, company-wide** clearing account (one
running balance in `Support!B16`), not per-customer or per-plot: every
approved interest/penalty payment and every overpayment debit decrements
it. This is closer to a general-ledger clearing account than the
per-payment "unallocated receipts" queue described in
[08](08-payments-and-receipting.md) — both concepts are legitimate and
the new design should probably support both: a per-account suspense
holding for unallocated/ambiguous receipts, *and* a GL-style clearing
account for interest/penalty income recognition.

When a loan balance goes negative (overpaid), `frm_Overpayment` lets an
officer debit the credit balance for one of four specific, pre-set
reasons: **Transfer to Title, Refund to Customer, Pay Debt for Another
Plot, Pay Debt for Another Customer's Plot.** That last two — applying one
customer's credit to a *different* plot or a *different* customer's debt
— is a concrete cross-account credit-transfer capability not currently
called out in [08](08-payments-and-receipting.md) and worth adding
explicitly rather than only generically covering "overpayments held as
credit."

## 7. Reporting reality

- **Executive KPIs** (`frm_MAIN` dashboard, values precomputed by Excel
  formulas in `Support`): Total Customers, Total Projects, Total Plots,
  Total Loans (count + KES), Active Loans (count + KES), Active Loan Book,
  **Performing** vs **Non-Performing** (count + KES each) — filterable by
  year or "Since Inception." This maps directly onto
  [11](11-reports-and-analytics.md)'s Executive Dashboard; Performing/
  Non-Performing is the legacy system's only arrears bucket (binary, no
  ageing tiers).
- **Arrears monitoring** (`frm_MONITORING`): a flat searchable list of
  every loan, with rows turning **red when a days-overdue column exceeds
  30** and black once status = `Closed Loan Account`. One threshold, no
  configurable buckets — validates the need for
  [08](08-payments-and-receipting.md)'s configurable ageing buckets, but
  confirms 30 days is the business's existing informal "something's
  wrong" line.
- **Reporting screen** (`frm_REPORTING`): filters the single `PrintLoan`
  ledger by category (Interest/Payments/Penalty) or date range and
  exports to `#Extracts(Reports)/`. One ledger, many views — a pattern
  worth keeping (see [08](08-payments-and-receipting.md)'s reconciliation
  invariant: everything should trace to one transaction ledger).
- **Daily Activity Report** (`frm_DAILYREPORT`, exported to
  `#DailyReportPDF/`): a **per-staff productivity report** not currently
  in [11](11-reports-and-analytics.md) — Site Visits, Savings Made, Sales
  Made, New Business Leads, Office Visits, Walk-Ins, Instalments Made,
  Transfers, captured per staff member per day. Add this as an explicit
  report type (a lightweight CRM/activity log, distinct from financial
  reporting).
- **Customer feedback** (`Comments` sheet, embedded in both `frm_MAIN` and
  `frm_Finance`): type (Rating/Question/Complaint/Other), free text,
  gender, 1–5 star rating. Not represented anywhere in docs 01–15 — a
  genuine net-new feature to decide in/out for the new platform rather
  than an oversight to fix.

## 8. Security reality

- **Authentication**: up to 5 hardcoded username/password pairs stored in
  plaintext in a worksheet (`SetUps!B2:C6`). No hashing, no lockout, no
  password policy, no MFA.
- **Authorization**: exactly one privileged check in the entire codebase —
  `If frm_LogIn.ComboBox1.Value <> "Admin" Then MsgBox "You have no
  Rights!"` gating the user-management screen. Every other screen is open
  to any logged-in user; there is no field masking (cost price, minimum
  price, margins are all visible to everyone) and no per-project or
  per-branch scoping (single tenant, single "location" for the whole
  business).
- **Audit trail**: `S_Activities` logs screen navigation and login/logout
  (who opened which form, when) — it does **not** log field-level
  before/after changes to data. There is no way to answer "who changed
  this plot's price and what was it before" from the legacy system.
- **File attachments**: KYC photos (face, ID, KRA cert) are copied into a
  shared `#images/` folder with timestamp-based filenames and referenced
  by full local file path in `CustomerInfo` — this only works because
  everyone shares the same local file, which is precisely the
  single-user constraint the new platform removes.

**Planned improvement**: this whole section is the concrete justification
for [10](10-database-and-security-design.md)'s hashed credentials,
role/branch/project-scoped RBAC, field-level masking, and full before/
after audit logging — none of it is present today, so the new platform
isn't just reimplementing security, it's adding it.

## 9. Integrations are UI automation, not APIs

- **Email**: draws a draft via `CreateObject("Outlook.Application")` and
  calls `.Display` (not `.Send`) — a human still clicks Send in Outlook.
  Not a real email service integration.
- **WhatsApp**: opens `web.whatsapp.com` with a pre-filled number, then
  uses `SendKeys` to paste a screenshot of the statement into the chat —
  full UI automation against the Chrome window, extremely fragile
  (breaks if focus shifts, if WhatsApp Web's layout changes, or if the
  machine is locked).

**Planned improvement**: [12](12-api-and-integration-design.md)'s
notification-service-with-channel-adapters replaces both with real
integrations (transactional email provider, WhatsApp Business API)
instead of automating a human's screen.

## 10. Gap analysis — legacy behavior vs. planned platform

| Area | Legacy (today) | New platform | Doc |
|---|---|---|---|
| Auth | 5 hardcoded plaintext users | Hashed credentials, unlimited users, org-scoped | [10](10-database-and-security-design.md) |
| Roles | One hardcoded `"Admin"` check | Full RBAC: role/project/branch/field/action scoped | [04](04-user-roles-and-permissions.md) |
| Plot identity | Free-text description, globally unique | System UUID + configurable human code, per-project | [05](05-project-and-plot-management.md) |
| Sale modes | Everything is a manually-typed "loan" | Real Full Cash + interest-free/-bearing Lipa Pole Pole | [07](07-sales-and-booking-workflows.md), [08](08-payments-and-receipting.md) |
| Interest/schedule | Manual charges, no schedule | Configurable interest, generated recalculable schedule | [08](08-payments-and-receipting.md) |
| Approval | Single step, no separation of duties | N-level, role-scoped, conditional | [09](09-approval-workflows.md) |
| Suspense | One company-wide GL balance | Per-account suspense **and** GL clearing account | [08](08-payments-and-receipting.md) |
| Overpayment reallocation | 4 fixed reasons incl. cross-plot/customer transfer | Same capability, explicit in spec | [08](08-payments-and-receipting.md) |
| Arrears | Binary Performing/Non-Performing, 30-day flag | Configurable ageing buckets | [08](08-payments-and-receipting.md) |
| Audit | Navigation log only | Full before/after field-level audit | [10](10-database-and-security-design.md) |
| Map | None — plots have no coordinates at all | Interactive polygon map, versioned | [06](06-interactive-map-engine.md) |
| Notifications | Outlook draft + WhatsApp `SendKeys` | Real email/SMS/WhatsApp API integrations | [12](12-api-and-integration-design.md) |
| Daily activity report | Present (site visits, sales, leads, etc.) | **Add** to report library | [11](11-reports-and-analytics.md) |
| Customer feedback/ratings | Present (Comments sheet) | **Decide** in/out — not currently specified anywhere | — open question |
| Multi-tenant | Single business, single workbook | Multi-org, multi-branch | [10](10-database-and-security-design.md) |

## 11. Open question for the user

The legacy system has a working **customer feedback/ratings module**
(type, free text, gender, 1–5 rating) that nothing in the current spec
covers. Worth an explicit decision: carry it forward as a lightweight
feature, or intentionally drop it as out of scope for v1.
