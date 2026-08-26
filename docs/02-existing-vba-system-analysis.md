# 02 — Existing VBA System Analysis

**Status: not started.** This is the highest-value next research task —
everything else in `docs/` is inferred from conversation, not from the
actual legacy system's rules.

## Why this matters

The legacy workbook (`legacy-excel/PlotsManager/PPP_v.01.Xls.xlsm`) encodes
years of real business rules — numbering schemes, validation, statuses,
report layouts — that are easy to get wrong by guessing. It should be
treated as the functional spec of record until this analysis is done and
this document is filled in.

## Blocker: VBA export

VBA embedded in `.xlsm` is not readable as plain text by tooling. Before
analysis can happen, export from the Excel VBA editor (Alt+F11 →
right-click each module → Export File):

- `.bas` — standard modules
- `.cls` — class modules
- `.frm` + `.frx` — UserForms (the `.frx` holds binary form resources;
  export both)

Drop exports into `legacy-excel/exported-vba/`. That directory, like the
rest of `legacy-excel/`, is gitignored — it will contain the same
sensitive material as the workbook itself.

## What to extract once exported

- **Data model**: sheets/tables used for projects, plots, customers,
  bookings, payments — column meanings, data types, lookups.
- **Numbering rules**: how project codes, plot numbers, and any
  customer/payment references are generated (the `PL.<n>-<code>-<serial>`
  pattern visible in `legacy-excel/PlotsManager/#ExtractsPDF/` filenames is
  a strong hint, but should be confirmed against the actual macro).
- **Validation logic**: what the macros reject or require before saving a
  record.
- **Statuses and transitions**: what plot/booking states exist today and
  what triggers each transition.
- **User forms and workflows**: what a user actually clicks through to
  create a project, sell a plot, record a payment.
- **Reports and dashboards**: the layouts already in
  `legacy-excel/PlotsManager/#DailyReportPDF/`,
  `#ExtractsPDF/`, `#PaymentSchedulesPDF/`, and `#Extracts(Reports)/` are a
  ready-made requirements list for [11](11-reports-and-analytics.md) —
  match each existing report to a planned one.
- **Security logic**: is there any login/access control (a `LogIn.JPG` /
  `LoginIcon.JPG` asset exists in `PlotsManager/@/`, suggesting some form
  of login screen) — and how granular is it.
- **Gaps and pain points**: anything duplicated across sheets, manual
  reconciliation steps, or known limitations worth deliberately fixing
  rather than replicating.

## Handling sensitive data during analysis

Before deep analysis, work from a duplicate of the workbook with
passwords, credentials, private client documents, and live customer data
removed or replaced with synthetic data — per the original project
instructions. The current `legacy-excel/` copy in this repo has **not**
been scrubbed and must stay local-only (already gitignored).

## Output

Once exported and reviewed, replace this stub with the actual findings,
and update [03](03-functional-requirements.md)–[11](11-reports-and-analytics.md)
wherever the real rules differ from what's currently documented there.
