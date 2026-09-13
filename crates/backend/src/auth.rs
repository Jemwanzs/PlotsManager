//! Password hashing and session-token primitives the backend owns outright
//! (see docs/10-database-and-security-design.md — no Auth-as-a-service
//! dependency). Signup/login HTTP handlers aren't wired up yet — that's
//! the "Rust APIs & Authentication" roadmap phase — but these primitives
//! are complete and tested so that work is wiring, not building.

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("failed to hash password")]
    HashFailed,
    #[error("failed to parse stored password hash")]
    InvalidStoredHash,
    #[error("invalid or expired session token")]
    InvalidToken,
}

pub fn hash_password(plain: &str) -> Result<String, AuthError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(plain.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|_| AuthError::HashFailed)
}

pub fn verify_password(plain: &str, stored_hash: &str) -> Result<bool, AuthError> {
    let parsed = PasswordHash::new(stored_hash).map_err(|_| AuthError::InvalidStoredHash)?;
    Ok(Argon2::default()
        .verify_password(plain.as_bytes(), &parsed)
        .is_ok())
}

/// Session claims. `organization_id` rides in the token itself (not
/// re-derived from a lookup on every request) so the backend can set the
/// RLS defense-in-depth session variable
/// (`select set_config('app.current_organization_id', ...)`, see
/// database/migrations/0001_init.sql) without an extra query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    pub organization_id: Uuid,
    pub exp: i64,
}

pub fn issue_session_token(
    user_id: Uuid,
    organization_id: Uuid,
    secret: &str,
    ttl: Duration,
) -> Result<String, AuthError> {
    let claims = Claims {
        sub: user_id,
        organization_id,
        exp: (Utc::now() + ttl).timestamp(),
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|_| AuthError::InvalidToken)
}

pub fn verify_session_token(token: &str, secret: &str) -> Result<Claims, AuthError> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|_| AuthError::InvalidToken)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_hash_roundtrip() {
        let hash = hash_password("correct horse battery staple").unwrap();
        assert!(verify_password("correct horse battery staple", &hash).unwrap());
        assert!(!verify_password("wrong password", &hash).unwrap());
    }

    #[test]
    fn two_hashes_of_the_same_password_differ() {
        // Argon2 salts are random per call, so this also guards against a
        // regression that reuses a fixed salt.
        let a = hash_password("same password").unwrap();
        let b = hash_password("same password").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn session_token_roundtrip() {
        let user_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();
        let token =
            issue_session_token(user_id, org_id, "test-secret", Duration::hours(1)).unwrap();

        let claims = verify_session_token(&token, "test-secret").unwrap();
        assert_eq!(claims.sub, user_id);
        assert_eq!(claims.organization_id, org_id);
    }

    #[test]
    fn session_token_rejects_wrong_secret() {
        let token =
            issue_session_token(Uuid::new_v4(), Uuid::new_v4(), "right-secret", Duration::hours(1))
                .unwrap();
        assert!(verify_session_token(&token, "wrong-secret").is_err());
    }

    #[test]
    fn session_token_rejects_expired() {
        let token = issue_session_token(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "test-secret",
            Duration::seconds(-1),
        )
        .unwrap();
        assert!(verify_session_token(&token, "test-secret").is_err());
    }
}
