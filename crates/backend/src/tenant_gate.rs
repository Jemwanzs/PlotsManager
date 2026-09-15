//! The tenant-access rule shared by login (`routes/auth.rs`) and every
//! authenticated request (`extractors.rs`'s `AuthUser`): a deactivated
//! organization can't be used, and a tenant whose trial has expired
//! can't either — except the platform owner's own organization, which is
//! exempt by construction. One function so the two call sites can't
//! silently drift apart on the exact conditions or wording.
use chrono::{DateTime, Utc};

use crate::error::AppError;

pub fn check(
    org_status: &str,
    is_platform_owner: bool,
    subscription_status: Option<&str>,
    trial_ends_at: Option<DateTime<Utc>>,
) -> Result<(), AppError> {
    if org_status == "deactivated" {
        return Err(AppError::forbidden(
            "This organization's account has been deactivated. Contact your platform administrator.",
        ));
    }

    // A tenant with no organization_subscriptions row at all (true of
    // pre-existing dev/demo data from before migration 0004) is left
    // unrestricted rather than locked out.
    if !is_platform_owner {
        if let (Some(status), Some(trial_ends_at)) = (subscription_status, trial_ends_at) {
            if status == "trialing" && trial_ends_at < Utc::now() {
                return Err(AppError::forbidden(
                    "Your trial period has expired. Contact us to continue using Real Estate Manager.",
                ));
            }
        }
    }

    Ok(())
}
