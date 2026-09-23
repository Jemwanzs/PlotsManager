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
    pub days_in_arrears: i32,
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
