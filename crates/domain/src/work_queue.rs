//! `GET /api/v1/work-queue` — the "what needs my attention today"
//! aggregate the roadmap flagged as missing (Phase 6B): loans in
//! arrears, quotations about to expire, pending price-approval
//! requests, leads with a follow-up due, and (platform owner only)
//! tenant applications awaiting approval, all in one list instead of
//! five separate reports nobody checks daily. Purely a read
//! aggregation over data that already exists — no new stored state,
//! nothing here can go stale independently of the rows it reads.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkQueueItemKind {
    LoanArrears,
    QuotationExpiring,
    ApprovalPending,
    LeadFollowUp,
    TenantPendingApproval,
}

impl WorkQueueItemKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::LoanArrears => "Loan in arrears",
            Self::QuotationExpiring => "Quotation expiring",
            Self::ApprovalPending => "Approval pending",
            Self::LeadFollowUp => "Follow-up due",
            Self::TenantPendingApproval => "Tenant awaiting approval",
        }
    }

    pub fn color(self) -> &'static str {
        match self {
            Self::LoanArrears => "#dc2626",
            Self::QuotationExpiring => "#d97706",
            Self::ApprovalPending => "#2563eb",
            Self::LeadFollowUp => "#7c3aed",
            Self::TenantPendingApproval => "#0891b2",
        }
    }
}

/// One actionable row. `amount`, not a pre-formatted string — the
/// frontend already has `format_amount`/a currency note per section,
/// and baking a formatted figure into the backend response would
/// bypass that (and hardcode a currency the backend doesn't own the
/// display convention for). `days_overdue` is only set once `due_date`
/// has actually passed; a not-yet-due item (an expiring quotation
/// still inside its lookahead window, e.g.) carries `due_date` alone.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkQueueItem {
    pub kind: WorkQueueItemKind,
    pub title: String,
    pub subtitle: String,
    pub amount: Option<Decimal>,
    pub href: String,
    pub due_date: Option<NaiveDate>,
    pub days_overdue: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkQueue {
    pub items: Vec<WorkQueueItem>,
}
