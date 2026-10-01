use serde::Deserialize;

pub const REPOSITORY: &str = "vrchat/packages";
pub const MAX_RELEASE_PAGES: u32 = 10;
pub const GITHUB_JSON_ACCEPT: &str = "application/vnd.github+json";
pub const GITHUB_ASSET_ACCEPT: &str = "application/octet-stream";
pub const DEFAULT_GITHUB_API_VERSION: &str = "2022-11-28";
pub const DEFAULT_USER_AGENT: &str = "vrc-udon-methods-sdk-proxy";
pub const DEFAULT_CACHE_TTL_SECONDS: u64 = 300;
pub const DEFAULT_STALE_REVALIDATE_SECONDS: u64 = 86_400;
pub const DEFAULT_TOKEN_REFRESH_SKEW_SECONDS: u64 = 300;
pub const DEFAULT_JWT_BACKDATE_SECONDS: u64 = 60;
pub const DEFAULT_JWT_LIFETIME_SECONDS: u64 = 540;
pub const MAX_LOGGED_BODY_BYTES: usize = 2048;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct GithubRelease {
    pub tag_name: String,
    #[serde(default)]
    pub assets: Vec<GithubAsset>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct GithubAsset {
    pub name: String,
    pub browser_download_url: String,
    #[serde(default)]
    pub url: Option<String>,
}
