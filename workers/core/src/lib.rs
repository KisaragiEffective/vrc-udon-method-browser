#![deny(clippy::all)]
#![warn(clippy::nursery)]

mod cache;
mod config;
mod github;
mod logging;
mod token;
mod versions;

pub use cache::{
    CacheOutcome, CacheReadDecision, CacheWriteDecision, CachedGithubResponse,
    GithubResponseForCache, cache_read_decision, cache_write_decision, github_cache_key,
    is_cacheable_github_endpoint,
};
pub use config::{ConfigurationError, REQUIRED_BINDINGS, validate_required_bindings};
pub use github::{
    DEFAULT_CACHE_TTL_SECONDS, DEFAULT_GITHUB_API_VERSION, DEFAULT_JWT_BACKDATE_SECONDS,
    DEFAULT_JWT_LIFETIME_SECONDS, DEFAULT_STALE_REVALIDATE_SECONDS,
    DEFAULT_TOKEN_REFRESH_SKEW_SECONDS, DEFAULT_USER_AGENT, GITHUB_ASSET_ACCEPT,
    GITHUB_JSON_ACCEPT, GithubAsset, GithubRelease, MAX_LOGGED_BODY_BYTES, MAX_RELEASE_PAGES,
    REPOSITORY,
};
pub use logging::redact_log_body;
pub use token::{
    TokenRefreshPolicy, can_use_old_token_after_refresh_failure, should_refresh_token,
};
pub use versions::{
    WORLD_ASSET_PREFIX, WORLD_ASSET_SUFFIX, WorldSdkVersion, collect_world_sdk_versions,
    find_world_sdk_asset, github_releases_url, is_valid_version, sort_world_sdk_versions_desc,
    version_from_asset_name,
};
