use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// SaaS subscription billing for an organization's use of the platform
/// itself — distinct from `sales::Payment`, which is a customer paying for
/// a plot. See docs/16-billing-and-subscriptions.md.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BillingInterval {
    Monthly,
    Annual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionPlan {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub price: Decimal,
    pub currency: String,
    pub billing_interval: BillingInterval,
    pub paystack_plan_code: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionStatus {
    Incomplete,
    Trialing,
    Active,
    PastDue,
    Cancelled,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationSubscription {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub plan_id: Uuid,
    pub paystack_customer_code: Option<String>,
    pub paystack_subscription_code: Option<String>,
    pub status: SubscriptionStatus,
    pub current_period_start: Option<DateTime<Utc>>,
    pub current_period_end: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvoiceStatus {
    Pending,
    Paid,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingInvoice {
    pub id: Uuid,
    pub organization_subscription_id: Uuid,
    pub paystack_reference: String,
    pub amount: Decimal,
    pub currency: String,
    pub status: InvoiceStatus,
    pub paid_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}
