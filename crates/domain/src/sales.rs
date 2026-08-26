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
