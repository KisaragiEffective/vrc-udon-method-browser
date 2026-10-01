use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::github::GITHUB_JSON_ACCEPT;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CachedGithubResponse {
    pub body: String,
    pub status: u16,
    pub etag: Option<String>,
    pub fetched_at_ms: u64,
    pub expires_at_ms: u64,
    pub method: String,
    pub url: String,
    pub accept: String,
    pub api_version: String,
}

impl CachedGithubResponse {
    #[must_use]
    pub const fn is_fresh(&self, now_ms: u64) -> bool {
        now_ms < self.expires_at_ms
    }

    #[must_use]
    pub fn refreshed_from_304(&self, now_ms: u64, ttl_seconds: u64) -> Self {
        let mut refreshed = self.clone();
        refreshed.fetched_at_ms = now_ms;
        refreshed.expires_at_ms = now_ms + ttl_seconds.saturating_mul(1000);
        refreshed
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheReadDecision {
    FreshHit(CachedGithubResponse),
    Revalidate {
        cached: CachedGithubResponse,
        etag: Option<String>,
    },
    Miss,
    Bypass,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheWriteDecision {
    Store(CachedGithubResponse),
    DoNotStore,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheOutcome {
    FreshHit,
    Miss,
    Bypass,
    StaleRevalidate304,
    Updated200,
    NotStored,
}

impl CacheOutcome {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::FreshHit => "fresh_hit",
            Self::Miss => "miss",
            Self::Bypass => "bypass",
            Self::StaleRevalidate304 => "stale_revalidate_304",
            Self::Updated200 => "updated_200",
            Self::NotStored => "not_stored",
        }
    }
}

#[must_use]
pub fn is_cacheable_github_endpoint(method: &str, url: &str, accept: &str) -> bool {
    if method != "GET" || accept != GITHUB_JSON_ACCEPT {
        return false;
    }

    let Some(rest) = url.strip_prefix("https://api.github.com/repos/vrchat/packages/releases?")
    else {
        return false;
    };

    let mut has_per_page = false;
    let mut has_page = false;
    for pair in rest.split('&') {
        let mut parts = pair.splitn(2, '=');
        match (parts.next(), parts.next()) {
            (Some("per_page"), Some("100")) => has_per_page = true,
            (Some("page"), Some(page)) => {
                has_page = page.parse::<u32>().is_ok_and(|page| page > 0);
            }
            _ => {}
        }
    }

    has_per_page && has_page
}

#[must_use]
pub fn github_cache_key(method: &str, url: &str, accept: &str, api_version: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(method.as_bytes());
    hasher.update(b"\n");
    hasher.update(url.as_bytes());
    hasher.update(b"\n");
    hasher.update(accept.as_bytes());
    hasher.update(b"\n");
    hasher.update(api_version.as_bytes());
    hasher.update(b"\ninstallation");
    format!("github-cache:v1:{}", hex::encode(hasher.finalize()))
}

#[must_use]
pub fn cache_read_decision(
    method: &str,
    url: &str,
    accept: &str,
    now_ms: u64,
    cached: Option<CachedGithubResponse>,
) -> CacheReadDecision {
    if !is_cacheable_github_endpoint(method, url, accept) {
        return CacheReadDecision::Bypass;
    }

    match cached {
        Some(cached) if cached.is_fresh(now_ms) => CacheReadDecision::FreshHit(cached),
        Some(cached) => CacheReadDecision::Revalidate {
            etag: cached.etag.clone(),
            cached,
        },
        None => CacheReadDecision::Miss,
    }
}

pub struct GithubResponseForCache<'a> {
    pub status: u16,
    pub body: &'a str,
    pub etag: Option<String>,
    pub method: &'a str,
    pub url: &'a str,
    pub accept: &'a str,
    pub api_version: &'a str,
}

#[must_use]
pub fn cache_write_decision(
    response: GithubResponseForCache<'_>,
    now_ms: u64,
    ttl_seconds: u64,
) -> CacheWriteDecision {
    if response.status != 200
        || !is_cacheable_github_endpoint(response.method, response.url, response.accept)
    {
        return CacheWriteDecision::DoNotStore;
    }

    CacheWriteDecision::Store(CachedGithubResponse {
        body: response.body.to_owned(),
        status: response.status,
        etag: response.etag,
        fetched_at_ms: now_ms,
        expires_at_ms: now_ms + ttl_seconds.saturating_mul(1000),
        method: response.method.to_owned(),
        url: response.url.to_owned(),
        accept: response.accept.to_owned(),
        api_version: response.api_version.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use crate::{
        CacheReadDecision, CacheWriteDecision, CachedGithubResponse, DEFAULT_GITHUB_API_VERSION,
        GITHUB_ASSET_ACCEPT, GITHUB_JSON_ACCEPT, GithubResponseForCache, cache_read_decision,
        cache_write_decision, github_cache_key, github_releases_url, is_cacheable_github_endpoint,
    };

    #[test]
    fn cache_allowlist_only_accepts_releases_json_gets() {
        let url = github_releases_url(1);

        assert!(is_cacheable_github_endpoint(
            "GET",
            &url,
            GITHUB_JSON_ACCEPT
        ));
        assert!(!is_cacheable_github_endpoint(
            "POST",
            &url,
            GITHUB_JSON_ACCEPT
        ));
        assert!(!is_cacheable_github_endpoint(
            "GET",
            &url,
            GITHUB_ASSET_ACCEPT
        ));
        assert!(!is_cacheable_github_endpoint(
            "GET",
            "https://api.github.com/repos/vrchat/packages",
            GITHUB_JSON_ACCEPT
        ));
    }

    #[test]
    fn cache_key_changes_with_request_shape() {
        let url = github_releases_url(1);
        let key = github_cache_key("GET", &url, GITHUB_JSON_ACCEPT, DEFAULT_GITHUB_API_VERSION);

        assert_ne!(
            key,
            github_cache_key(
                "GET",
                &github_releases_url(2),
                GITHUB_JSON_ACCEPT,
                DEFAULT_GITHUB_API_VERSION
            )
        );
        assert_ne!(
            key,
            github_cache_key("GET", &url, GITHUB_ASSET_ACCEPT, DEFAULT_GITHUB_API_VERSION)
        );
        assert_ne!(
            key,
            github_cache_key("GET", &url, GITHUB_JSON_ACCEPT, "2023-01-01")
        );
    }

    #[test]
    fn fresh_cache_hit_avoids_revalidation() {
        let cached = CachedGithubResponse {
            body: "[]".to_owned(),
            status: 200,
            etag: Some("\"abc\"".to_owned()),
            fetched_at_ms: 1_000,
            expires_at_ms: 11_000,
            method: "GET".to_owned(),
            url: github_releases_url(1),
            accept: GITHUB_JSON_ACCEPT.to_owned(),
            api_version: DEFAULT_GITHUB_API_VERSION.to_owned(),
        };

        assert!(matches!(
            cache_read_decision(
                "GET",
                &github_releases_url(1),
                GITHUB_JSON_ACCEPT,
                5_000,
                Some(cached)
            ),
            CacheReadDecision::FreshHit(_)
        ));
    }

    #[test]
    fn stale_cache_uses_etag_for_revalidation() {
        let cached = CachedGithubResponse {
            body: "[1]".to_owned(),
            status: 200,
            etag: Some("\"abc\"".to_owned()),
            fetched_at_ms: 1_000,
            expires_at_ms: 2_000,
            method: "GET".to_owned(),
            url: github_releases_url(1),
            accept: GITHUB_JSON_ACCEPT.to_owned(),
            api_version: DEFAULT_GITHUB_API_VERSION.to_owned(),
        };

        let decision = cache_read_decision(
            "GET",
            &github_releases_url(1),
            GITHUB_JSON_ACCEPT,
            5_000,
            Some(cached.clone()),
        );

        assert_eq!(
            decision,
            CacheReadDecision::Revalidate {
                cached,
                etag: Some("\"abc\"".to_owned())
            }
        );
    }

    #[test]
    fn not_modified_refreshes_saved_body_ttl() {
        let cached = CachedGithubResponse {
            body: "[1]".to_owned(),
            status: 200,
            etag: Some("\"abc\"".to_owned()),
            fetched_at_ms: 1_000,
            expires_at_ms: 2_000,
            method: "GET".to_owned(),
            url: github_releases_url(1),
            accept: GITHUB_JSON_ACCEPT.to_owned(),
            api_version: DEFAULT_GITHUB_API_VERSION.to_owned(),
        };

        let refreshed = cached.refreshed_from_304(5_000, 60);

        assert_eq!(refreshed.body, "[1]");
        assert_eq!(refreshed.fetched_at_ms, 5_000);
        assert_eq!(refreshed.expires_at_ms, 65_000);
    }

    #[test]
    fn ok_response_replaces_cache() {
        let decision = cache_write_decision(
            GithubResponseForCache {
                status: 200,
                body: "[2]",
                etag: Some("\"new\"".to_owned()),
                method: "GET",
                url: &github_releases_url(1),
                accept: GITHUB_JSON_ACCEPT,
                api_version: DEFAULT_GITHUB_API_VERSION,
            },
            10_000,
            30,
        );

        let CacheWriteDecision::Store(cached) = decision else {
            panic!("expected cache store");
        };
        assert_eq!(cached.body, "[2]");
        assert_eq!(cached.etag, Some("\"new\"".to_owned()));
        assert_eq!(cached.expires_at_ms, 40_000);
    }

    #[test]
    fn non_ok_response_is_not_cached() {
        assert_eq!(
            cache_write_decision(
                GithubResponseForCache {
                    status: 500,
                    body: "{}",
                    etag: None,
                    method: "GET",
                    url: &github_releases_url(1),
                    accept: GITHUB_JSON_ACCEPT,
                    api_version: DEFAULT_GITHUB_API_VERSION,
                },
                10_000,
                30,
            ),
            CacheWriteDecision::DoNotStore
        );
    }
}
