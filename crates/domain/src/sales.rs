use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentMode {
    FullCash,
    LipaPolePoleInterestFree,
    LipaPolePoleInterestBearing,
}

/// Status of a Plot Loan Account — the receivable/repayment account created
/// for an instalment sale. Secured against the plot, not a disbursed cash loan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoanAccountStatus {
    Draft,
    PendingApproval,
    ApprovedAwaitingDeposit,
    ActiveCurrent,
    ActivePartiallyPaid,
    InGracePeriod,
    InArrears,
    Restructured,
    SettlementPendingVerification,
    FullyPaid,
    Cancelled,
    Defaulted,
    RepossessedOrReallocated,
    Closed,
}

/// A sale's own lifecycle, distinct from the plot's or the loan
/// account's — added alongside both rather than folded into either,
/// since "was this sale cancelled" is a fact about the transaction
/// itself, and a plot or loan account can be re-created fresh against
/// a new sale afterward. See `database/migrations/
/// 0030_sale_lifecycle.sql`'s module docs for why `plot_sales`/
/// `sale_plots`'s uniqueness constraints had to change to allow this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SaleLifecycleStatus {
    Active,
    Cancelled,
    Repossessed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlotSale {
    pub id: Uuid,
    pub plot_id: Uuid,
    pub customer_id: Uuid,
    pub organization_id: Uuid,
    pub agent_id: Option<Uuid>,
    pub payment_mode: PaymentMode,
    pub agreed_price: Decimal,
    pub created_at: DateTime<Utc>,
}

/// The receivable account for an instalment (Lipa Pole Pole) sale.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlotLoanAccount {
    pub id: Uuid,
    pub account_number: String,
    pub sale_id: Uuid,
    pub principal: Decimal,
    pub interest_rate: Option<Decimal>,
    pub deposit_required: Decimal,
    pub deposit_paid: Decimal,
    pub instalment_amount: Decimal,
    pub repayment_frequency_days: i32,
    pub start_date: NaiveDate,
    pub status: LoanAccountStatus,
    pub amount_paid: Decimal,
    pub outstanding_balance: Decimal,
    /// Derived live from `repayment_schedule_entries` on every read
    /// (never a stored, potentially-stale value) — age in days of the
    /// oldest schedule instalment that's still short of its `total_due`
    /// past the grace period. 0 when nothing's overdue.
    pub days_in_arrears: i32,
    pub next_instalment_due_date: Option<NaiveDate>,
    pub next_instalment_amount: Option<Decimal>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstalmentStatus {
    Upcoming,
    Due,
    PartiallyPaid,
    Paid,
    Overdue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepaymentScheduleEntry {
    pub id: Uuid,
    pub loan_account_id: Uuid,
    pub instalment_number: i32,
    pub due_date: NaiveDate,
    pub principal_due: Decimal,
    pub interest_due: Decimal,
    pub fees_due: Decimal,
    pub total_due: Decimal,
    pub amount_paid: Decimal,
    pub status: InstalmentStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentStatus {
    Captured,
    Verified,
    Posted,
    Rejected,
    Reversed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Payment {
    pub id: Uuid,
    pub loan_account_id: Uuid,
    pub amount: Decimal,
    pub payment_date: NaiveDate,
    pub method: String,
    pub external_reference: Option<String>,
    pub status: PaymentStatus,
    pub captured_by: Uuid,
    pub verified_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    /// `"RCT-00001"`-style, a real Postgres sequence
    /// (`payment_receipt_number_seq`, `database/migrations/
    /// 0031_payment_receipts.sql`) — the per-payment proof-of-payment
    /// reference, distinct from the loan statement (a running summary
    /// across every transaction on the account).
    pub receipt_number: String,
}

/// One row of a loan account's transaction ledger
/// (`database/migrations/0020_loan_ledger.sql`) — every charge,
/// payment, waiver and reversal, in the order they happened. The
/// source of truth `LoanStatement` is built from; never reconstructed
/// from the account's current balance (see that migration's own docs
/// on why `payments` stays a separate, unchanged table underneath).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LedgerEntryType {
    Payment,
    ChargeInterest,
    ChargePenalty,
    WaiverInterest,
    WaiverPenalty,
    Reversal,
    Adjustment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoanLedgerEntry {
    pub id: Uuid,
    pub loan_account_id: Uuid,
    pub entry_type: LedgerEntryType,
    pub entry_date: NaiveDate,
    pub gross_amount: Decimal,
    /// Signed: negative reduces that component's outstanding (a
    /// payment or a waiver), positive increases it (a charge). Always
    /// sums to this entry's net effect on `balance_after` relative to
    /// the entry before it.
    pub principal_delta: Decimal,
    pub interest_delta: Decimal,
    pub penalty_delta: Decimal,
    pub balance_after: Decimal,
    pub method: Option<String>,
    pub external_reference: Option<String>,
    pub notes: Option<String>,
    pub created_by_name: String,
    pub created_at: DateTime<Utc>,
}

/// `GET /api/v1/loan-accounts/:id/statement` — the full running
/// statement for one receivable account: header context plus every
/// ledger entry in order. `Date Range | View | Download PDF | Export
/// Excel` (the filter/export affordances) are frontend concerns over
/// this same data, not separate backend concepts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoanStatement {
    pub account: PlotLoanAccount,
    pub plot_number: String,
    pub project_name: String,
    pub customer_name: String,
    pub agreed_price: Decimal,
    pub status_label: String,
    pub status_color: String,
    pub entries: Vec<LoanLedgerEntry>,
}

/// A manual interest or penalty charge against a receivable account
/// (`POST /api/v1/loan-accounts/:id/charges`) — the mechanic sections
/// 7/8 of the finance enhancement need (an account can actually owe
/// more than its principal). Automatic/scheduled charging (a
/// configured rate applied on a recurring basis) needs a job
/// scheduler this app doesn't have yet; this is the manual fallback
/// the spec itself calls for in the meantime, not a stand-in pretending
/// to be the automatic version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChargeType {
    Interest,
    Penalty,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostChargeInput {
    pub loan_account_id: Uuid,
    pub charge_type: ChargeType,
    pub amount: Decimal,
    pub charge_date: NaiveDate,
    pub reason: String,
}

/// `GET /api/v1/loan-accounts/:id/allocation-preview?amount=X` — what
/// a payment of this size *would* clear, before it's posted. Same
/// penalty -> interest -> principal waterfall `record_payment` itself
/// applies; a read-only preview over it, not a separate calculation
/// that could drift from what actually gets posted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentAllocationPreview {
    pub amount: Decimal,
    pub penalty_paid: Decimal,
    pub interest_paid: Decimal,
    pub principal_paid: Decimal,
    pub new_balance: Decimal,
}

/// Forgiving some or all of the interest/penalty a receivable has
/// accrued — a business decision (goodwill, negotiated settlement),
/// not error-correction. Distinct from reversing an entry: a waiver
/// isn't tied to any one past `charge_interest`/`charge_penalty`
/// entry, since accrued interest/penalty can come from several.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaiverType {
    Interest,
    Penalty,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostWaiverInput {
    pub loan_account_id: Uuid,
    pub waiver_type: WaiverType,
    pub amount: Decimal,
    pub waiver_date: NaiveDate,
    pub reason: String,
}

/// `POST /api/v1/loan-accounts/:id/ledger-entries/:entry_id/reverse` —
/// undoes one specific past entry (a payment entered in error, or a
/// charge that shouldn't have been posted), not a forgiveness of
/// current balance. See `WaiverType`'s doc comment for the distinction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReverseEntryInput {
    pub reason: String,
}

/// `POST /api/v1/loan-accounts/:id/repayment-holiday` — pushes every
/// not-yet-fully-paid schedule entry's `due_date` forward by
/// `holiday_days`, so an agreed pause doesn't get flagged as arrears.
/// Doesn't touch `outstanding_balance`/`amount_paid` — nothing owed
/// changes, only when it's next due.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyRepaymentHolidayInput {
    pub holiday_days: i32,
    pub reason: Option<String>,
}

/// `POST /api/v1/loan-accounts/:id/restructure` — re-amortizes the
/// remaining principal over a new instalment amount and/or frequency,
/// replacing the not-yet-fully-paid tail of the schedule from
/// `effective_date` (default: today) onward. Total owed is unchanged;
/// only the forward calendar is.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestructureLoanInput {
    pub new_instalment_amount: Decimal,
    pub new_repayment_frequency_days: Option<i32>,
    pub effective_date: Option<NaiveDate>,
    pub reason: Option<String>,
}
