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
    // The platform owner is exempt from every tenant-status restriction,
    // not just the trial-expiry one below — this check runs on every
    // authenticated request (`extractors.rs`'s `AuthUser`) including the
    // platform owner's own, so without this exemption a deactivated
    // status on the platform owner's own organization would lock the
    // one cross-tenant admin account out of the platform entirely,
    // including the `/platform/*` pages that would undo it. See
    // `routes/platform.rs::deactivate_organization`'s own guard against
    // ever setting that status in the first place — this is the
    // second, defense-in-depth layer for the same rule.
    if is_platform_owner {
        return Ok(());
    }

    match org_status {
        "deactivated" => {
            return Err(AppError::forbidden(
                "This organization's account has been deactivated. Contact your platform administrator.",
            ));
        }
        "pending_approval" => {
            return Err(AppError::forbidden(
                "Your account is awaiting activation. You will be notified once your workspace has been approved.",
            ));
        }
        "rejected" => {
            return Err(AppError::forbidden(
                "This registration was not approved. Contact us if you believe this is a mistake.",
            ));
        }
        "suspended" => {
            return Err(AppError::forbidden(
                "This organization's account is suspended. Contact your platform administrator.",
            ));
        }
        "terminated" => {
            return Err(AppError::forbidden(
                "This organization's account has been terminated.",
            ));
        }
        _ => {}
    }

    // A tenant with no organization_subscriptions row at all (true of
    // pre-existing dev/demo data from before migration 0004) is left
    // unrestricted rather than locked out.
    if let (Some(status), Some(trial_ends_at)) = (subscription_status, trial_ends_at) {
        if status == "trialing" && trial_ends_at < Utc::now() {
            return Err(AppError::forbidden(
                "Your trial period has expired. Contact us to continue using Real Estate Manager.",
            ));
        }
    }

    Ok(())
}
