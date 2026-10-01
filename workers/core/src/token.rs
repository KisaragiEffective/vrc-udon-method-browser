use crate::github::DEFAULT_TOKEN_REFRESH_SKEW_SECONDS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenRefreshPolicy {
    pub refresh_skew_seconds: u64,
    pub emergency_min_valid_seconds: u64,
}

impl Default for TokenRefreshPolicy {
    fn default() -> Self {
        Self {
            refresh_skew_seconds: DEFAULT_TOKEN_REFRESH_SKEW_SECONDS,
            emergency_min_valid_seconds: 60,
        }
    }
}

#[must_use]
pub const fn should_refresh_token(
    now_ms: u64,
    expires_at_ms: u64,
    policy: TokenRefreshPolicy,
) -> bool {
    now_ms.saturating_add(policy.refresh_skew_seconds.saturating_mul(1000)) >= expires_at_ms
}

#[must_use]
pub const fn can_use_old_token_after_refresh_failure(
    now_ms: u64,
    expires_at_ms: u64,
    policy: TokenRefreshPolicy,
) -> bool {
    now_ms.saturating_add(policy.emergency_min_valid_seconds.saturating_mul(1000)) < expires_at_ms
}

#[cfg(test)]
mod tests {
    use super::{
        TokenRefreshPolicy, can_use_old_token_after_refresh_failure, should_refresh_token,
    };

    #[test]
    fn token_refresh_policy_respects_skew_and_emergency_window() {
        let policy = TokenRefreshPolicy::default();

        assert!(!should_refresh_token(1_000, 400_000, policy));
        assert!(should_refresh_token(100_000, 350_000, policy));
        assert!(can_use_old_token_after_refresh_failure(
            1_000, 100_000, policy
        ));
        assert!(!can_use_old_token_after_refresh_failure(
            50_000, 100_000, policy
        ));
    }
}
