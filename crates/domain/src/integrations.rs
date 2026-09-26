//! Settings-driven, per-category third-party integration configuration
//! (`database/migrations/0035_integration_configs.sql`) — the
//! infrastructure every actual provider integration plugs into later,
//! built ahead of any of those providers themselves (2026-09-26
//! decision, `docs/14-development-roadmap.md`). Nothing here sends an
//! SMS, an email, or a payment — it's just where an organization tells
//! the platform *how* it would, once that provider code exists.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationCategory {
    Sms,
    Email,
    Whatsapp,
    Payment,
    Banking,
    Accounting,
}

impl IntegrationCategory {
    /// The exact `snake_case` wire/path form (matches this enum's own
    /// `#[serde(rename_all = "snake_case")]`) — for building a URL
    /// path segment without a `serde_json` round trip at every call
    /// site.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sms => "sms",
            Self::Email => "email",
            Self::Whatsapp => "whatsapp",
            Self::Payment => "payment",
            Self::Banking => "banking",
            Self::Accounting => "accounting",
        }
    }

    pub const ALL: [IntegrationCategory; 6] = [
        Self::Sms,
        Self::Email,
        Self::Whatsapp,
        Self::Payment,
        Self::Banking,
        Self::Accounting,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Sms => "SMS",
            Self::Email => "Email",
            Self::Whatsapp => "WhatsApp",
            Self::Payment => "Payment / Mobile Money",
            Self::Banking => "Banking",
            Self::Accounting => "Accounting",
        }
    }
}

/// One organization's configuration for one category — at most one
/// provider live per category at a time (see the migration's own doc
/// comment on why). Never carries the actual secret values back out;
/// `has_api_key`/`has_api_secret` are all a caller gets, the same
/// write-only convention this app already uses for passwords.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationConfig {
    pub id: Uuid,
    pub category: IntegrationCategory,
    pub provider: String,
    pub enabled: bool,
    pub config: JsonValue,
    pub has_api_key: bool,
    pub has_api_secret: bool,
    pub has_extra_credentials: bool,
    pub updated_at: DateTime<Utc>,
    pub updated_by_name: Option<String>,
}

/// `PUT /api/v1/settings/integrations/:category` — upserts the config
/// for that category. `api_key`/`api_secret`/`extra_credentials` are
/// each tri-state by omission-vs-presence at the JSON level, not just
/// `Option`: leave the field out of the request entirely to keep
/// whatever's already stored, send `null` (`Some(None)` here once
/// deserialized... see the field docs below) to clear it, or send a
/// new value to replace it. Frontend forms only ever need "leave
/// blank to keep the existing one," which is exactly the omitted case.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertIntegrationConfigInput {
    pub provider: String,
    pub enabled: bool,
    #[serde(default)]
    pub config: JsonValue,
    /// Omitted (`#[serde(default)]` -> `None`): keep the existing
    /// value. `Some(s)`: replace it — an empty string clears it.
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub api_secret: Option<String>,
    /// Same tri-state convention, but a JSON object/array rather than
    /// a string — free-form bag for whatever a provider needs beyond a
    /// single key/secret pair (multiple tokens, a webhook secret,
    /// ...). `Some(JsonValue::Null)` clears it.
    #[serde(default)]
    pub extra_credentials: Option<JsonValue>,
}
