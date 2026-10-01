use serde::{Deserialize, Serialize};
use vrc_udon_methods_worker_core::{
    CacheOutcome, CacheReadDecision, CacheWriteDecision, CachedGithubResponse,
    GithubResponseForCache, MAX_LOGGED_BODY_BYTES, cache_read_decision, cache_write_decision,
    github_cache_key, redact_log_body,
};
use worker::{ByteStream, Env, Fetch, Headers, Method, Request, RequestInit, Result, console_log};

use crate::config::RuntimeConfig;
use crate::time::now_ms;

const CACHE_BINDING: &str = "GITHUB_API_CACHE";
const TOKEN_BROKER_BINDING: &str = "GITHUB_TOKEN_BROKER";
const TOKEN_BROKER_NAME: &str = "installation-token";

#[derive(Clone)]
pub struct GithubClient {
    env: Env,
    config: RuntimeConfig,
}

pub struct GithubStream {
    pub stream: ByteStream,
    pub etag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedGithubAssetHeaders {
    etag: String,
    fetched_at_ms: u64,
    method: String,
    url: String,
    accept: String,
    api_version: String,
}

impl GithubClient {
    pub const fn new(env: Env, config: RuntimeConfig) -> Self {
        Self { env, config }
    }

    pub async fn fetch_text(&self, url: &str, accept: &str, use_cache: bool) -> Result<String> {
        if use_cache && let Some(body) = self.cached_fetch_text(url, accept).await? {
            return Ok(body);
        }

        let (status, body, _) = self.fetch_text_uncached(url, accept, None, true).await?;
        if !(200..300).contains(&status) {
            return Err(format!("GitHub request failed: {status}").into());
        }
        Ok(body)
    }

    pub async fn fetch_stream(&self, url: &str, accept: &str) -> Result<GithubStream> {
        let key = github_cache_key("GET", url, accept, &self.config.api_version);
        let cached_etag = self.cached_asset_etag(&key).await?;
        let mut response = self
            .fetch_request(url, accept, cached_etag.as_deref(), true, true)
            .await?;
        let status = response.status_code();
        let mut headers = response.headers().clone();
        if status == 304 {
            self.log_github_response(url, status, &headers, None);
            self.log_cache(url, CacheOutcome::StaleRevalidate304);
            response = self.fetch_request(url, accept, None, true, false).await?;
            headers = response.headers().clone();
        }

        let status = response.status_code();
        if !(200..300).contains(&status) {
            let body = response.text().await?;
            self.log_github_response(url, status, &headers, Some(&body));
            return Err(format!("GitHub asset request failed ({status}): {body}").into());
        }

        let etag = headers.get("etag")?;
        self.store_asset_etag(&key, url, accept, etag.as_deref())
            .await?;
        self.log_github_response(url, status, &headers, None);
        Ok(GithubStream {
            stream: response.stream()?,
            etag,
        })
    }

    async fn cached_asset_etag(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .env
            .kv(CACHE_BINDING)?
            .get(key)
            .json::<CachedGithubAssetHeaders>()
            .await?
            .map(|cached| cached.etag))
    }

    async fn store_asset_etag(
        &self,
        key: &str,
        url: &str,
        accept: &str,
        etag: Option<&str>,
    ) -> Result<()> {
        let Some(etag) = etag else {
            self.log_cache(url, CacheOutcome::NotStored);
            return Ok(());
        };

        let cached = CachedGithubAssetHeaders {
            etag: etag.to_owned(),
            fetched_at_ms: now_ms(),
            method: "GET".to_owned(),
            url: url.to_owned(),
            accept: accept.to_owned(),
            api_version: self.config.api_version.clone(),
        };
        self.env
            .kv(CACHE_BINDING)?
            .put(key, serde_json::to_string(&cached)?)?
            .expiration_ttl(self.config.stale_revalidate_seconds)
            .execute()
            .await?;
        self.log_cache(url, CacheOutcome::Updated200);
        Ok(())
    }

    async fn cached_fetch_text(&self, url: &str, accept: &str) -> Result<Option<String>> {
        let kv = self.env.kv(CACHE_BINDING)?;
        let key = github_cache_key("GET", url, accept, &self.config.api_version);
        let now = now_ms();
        let cached = kv.get(&key).json::<CachedGithubResponse>().await?;

        match cache_read_decision("GET", url, accept, now, cached) {
            CacheReadDecision::FreshHit(cached) => {
                self.log_cache(url, CacheOutcome::FreshHit);
                Ok(Some(cached.body))
            }
            CacheReadDecision::Revalidate { cached, etag } => {
                self.revalidate_cached_text(url, accept, key, cached, etag.as_deref())
                    .await
            }
            CacheReadDecision::Miss => self.fetch_and_store_miss(url, accept, key).await,
            CacheReadDecision::Bypass => {
                self.log_cache(url, CacheOutcome::Bypass);
                Ok(None)
            }
        }
    }

    async fn revalidate_cached_text(
        &self,
        url: &str,
        accept: &str,
        key: String,
        cached: CachedGithubResponse,
        etag: Option<&str>,
    ) -> Result<Option<String>> {
        let kv = self.env.kv(CACHE_BINDING)?;
        let (status, body, response_etag) =
            self.fetch_text_uncached(url, accept, etag, true).await?;
        if status == 304 {
            let refreshed = cached.refreshed_from_304(now_ms(), self.config.cache_ttl_seconds);
            kv.put(&key, serde_json::to_string(&refreshed)?)?
                .expiration_ttl(self.config.stale_revalidate_seconds)
                .execute()
                .await?;
            self.log_cache(url, CacheOutcome::StaleRevalidate304);
            return Ok(Some(refreshed.body));
        }

        self.maybe_store_response(url, accept, &key, status, &body, response_etag)
            .await?;
        if !(200..300).contains(&status) {
            return Err(format!("GitHub request failed: {status}").into());
        }
        Ok(Some(body))
    }

    async fn fetch_and_store_miss(
        &self,
        url: &str,
        accept: &str,
        key: String,
    ) -> Result<Option<String>> {
        self.log_cache(url, CacheOutcome::Miss);
        let (status, body, etag) = self.fetch_text_uncached(url, accept, None, true).await?;
        self.maybe_store_response(url, accept, &key, status, &body, etag)
            .await?;

        if !(200..300).contains(&status) {
            return Err(format!("GitHub request failed: {status}").into());
        }
        Ok(Some(body))
    }

    async fn maybe_store_response(
        &self,
        url: &str,
        accept: &str,
        key: &str,
        status: u16,
        body: &str,
        etag: Option<String>,
    ) -> Result<()> {
        if let CacheWriteDecision::Store(next) = cache_write_decision(
            GithubResponseForCache {
                status,
                body,
                etag,
                method: "GET",
                url,
                accept,
                api_version: &self.config.api_version,
            },
            now_ms(),
            self.config.cache_ttl_seconds,
        ) {
            self.env
                .kv(CACHE_BINDING)?
                .put(key, serde_json::to_string(&next)?)?
                .expiration_ttl(self.config.stale_revalidate_seconds)
                .execute()
                .await?;
            self.log_cache(url, CacheOutcome::Updated200);
        } else {
            self.log_cache(url, CacheOutcome::NotStored);
        }
        Ok(())
    }

    async fn fetch_text_uncached(
        &self,
        url: &str,
        accept: &str,
        etag: Option<&str>,
        allow_retry: bool,
    ) -> Result<(u16, String, Option<String>)> {
        let mut response = self
            .fetch_request(url, accept, etag, true, allow_retry)
            .await?;
        let status = response.status_code();
        let headers = response.headers().clone();
        let body = if status == 304 {
            String::new()
        } else {
            response.text().await?
        };
        self.log_github_response(url, status, &headers, Some(&body));
        Ok((status, body, headers.get("etag")?))
    }

    async fn fetch_request(
        &self,
        url: &str,
        accept: &str,
        etag: Option<&str>,
        use_auth: bool,
        allow_retry: bool,
    ) -> Result<worker::Response> {
        let token = if use_auth {
            Some(self.installation_token().await?)
        } else {
            None
        };

        let mut request = Request::new(url, Method::Get)?;
        self.set_headers(request.headers_mut()?, accept, token.as_deref(), etag)?;
        let response = Fetch::Request(request).send().await?;

        if response.status_code() == 401 && use_auth && allow_retry {
            self.invalidate_installation_token().await?;
            let token = self.installation_token().await?;
            let mut request = Request::new(url, Method::Get)?;
            self.set_headers(request.headers_mut()?, accept, Some(&token), etag)?;
            return Fetch::Request(request).send().await;
        }

        Ok(response)
    }

    fn set_headers(
        &self,
        headers: &Headers,
        accept: &str,
        token: Option<&str>,
        etag: Option<&str>,
    ) -> Result<()> {
        headers.set("Accept", accept)?;
        headers.set("User-Agent", &self.config.user_agent)?;
        headers.set("X-GitHub-Api-Version", &self.config.api_version)?;
        if let Some(token) = token {
            headers.set("Authorization", &format!("Bearer {token}"))?;
        }
        if let Some(etag) = etag {
            headers.set("If-None-Match", etag)?;
        }
        Ok(())
    }

    async fn installation_token(&self) -> Result<String> {
        let namespace = self.env.durable_object(TOKEN_BROKER_BINDING)?;
        let stub = namespace.id_from_name(TOKEN_BROKER_NAME)?.get_stub()?;
        let mut response = stub
            .fetch_with_str("https://token-broker.local/token")
            .await?;
        if response.status_code() != 200 {
            return Err(format!("GitHub token broker failed: {}", response.status_code()).into());
        }
        let body = response.json::<TokenResponse>().await?;
        Ok(body.token)
    }

    async fn invalidate_installation_token(&self) -> Result<()> {
        let namespace = self.env.durable_object(TOKEN_BROKER_BINDING)?;
        let stub = namespace.id_from_name(TOKEN_BROKER_NAME)?.get_stub()?;
        let mut init = RequestInit::new();
        init.with_method(Method::Post);
        let request = Request::new_with_init("https://token-broker.local/invalidate", &init)?;
        let _response = stub.fetch_with_request(request).await?;
        Ok(())
    }

    fn log_cache(&self, url: &str, outcome: CacheOutcome) {
        console_log!("github_cache url={} outcome={}", url, outcome.as_str());
    }

    fn log_github_response(&self, url: &str, status: u16, headers: &Headers, body: Option<&str>) {
        let request_id = header_or_default(headers, "x-github-request-id");
        let limit = header_or_default(headers, "x-ratelimit-limit");
        let remaining = header_or_default(headers, "x-ratelimit-remaining");
        let reset = header_or_default(headers, "x-ratelimit-reset");
        console_log!(
            "github_api url={} status={} request_id={} rate_limit={} rate_remaining={} rate_reset={}",
            url,
            status,
            request_id,
            limit,
            remaining,
            reset
        );
        if self.config.log_body
            && let Some(body) = body
        {
            console_log!(
                "github_api_body url={} body={}",
                url,
                redact_log_body(body, MAX_LOGGED_BODY_BYTES)
            );
        }
    }
}

#[derive(Deserialize)]
struct TokenResponse {
    token: String,
}

pub fn header_or_default(headers: &Headers, name: &str) -> String {
    headers.get(name).ok().flatten().unwrap_or_default()
}
