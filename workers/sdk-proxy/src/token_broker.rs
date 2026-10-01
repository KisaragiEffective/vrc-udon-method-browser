use std::cell::RefCell;

use serde::{Deserialize, Serialize};
use vrc_udon_methods_worker_core::{
    DEFAULT_GITHUB_API_VERSION, DEFAULT_TOKEN_REFRESH_SKEW_SECONDS, DEFAULT_USER_AGENT,
    GITHUB_JSON_ACCEPT, TokenRefreshPolicy, can_use_old_token_after_refresh_failure,
    should_refresh_token,
};
use worker::{
    DurableObject, Env, Fetch, Headers, Method, Request, RequestInit, Response, Result, State,
    console_log, durable_object,
};

use crate::config::env_u64;
use crate::github_client::header_or_default;
use crate::jwt::create_app_jwt;
use crate::time::{now_ms, parse_github_time_ms};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InstallationTokenState {
    token: String,
    expires_at_ms: u64,
}

#[derive(Debug, Deserialize)]
struct GithubInstallationTokenResponse {
    token: String,
    expires_at: String,
}

#[durable_object]
pub struct GithubTokenBroker {
    env: Env,
    token: RefCell<Option<InstallationTokenState>>,
}

impl DurableObject for GithubTokenBroker {
    fn new(_state: State, env: Env) -> Self {
        Self {
            env,
            token: RefCell::new(None),
        }
    }

    async fn fetch(&self, req: Request) -> Result<Response> {
        let url = req.url()?;
        match (req.method(), url.path()) {
            (Method::Get, "/token") => {
                let token = self.get_token().await?;
                Response::from_json(&TokenResponseBody {
                    token: token.token,
                    expires_at_ms: token.expires_at_ms,
                })
            }
            (Method::Post, "/invalidate") => {
                *self.token.borrow_mut() = None;
                Response::empty()
            }
            _ => Response::error("not found", 404),
        }
    }
}

impl GithubTokenBroker {
    async fn get_token(&self) -> Result<InstallationTokenState> {
        let now = now_ms();
        let policy = TokenRefreshPolicy {
            refresh_skew_seconds: env_u64(
                &self.env,
                "GITHUB_TOKEN_REFRESH_SKEW_SECONDS",
                DEFAULT_TOKEN_REFRESH_SKEW_SECONDS,
            ),
            emergency_min_valid_seconds: 60,
        };

        if let Some(token) = self.token.borrow().as_ref()
            && !should_refresh_token(now, token.expires_at_ms, policy)
        {
            return Ok(token.clone());
        }

        match self.issue_token().await {
            Ok(token) => {
                *self.token.borrow_mut() = Some(token.clone());
                Ok(token)
            }
            Err(error) => {
                if let Some(token) = self.token.borrow().as_ref()
                    && can_use_old_token_after_refresh_failure(now, token.expires_at_ms, policy)
                {
                    console_log!("github_token_refresh_failed_using_still_valid_token");
                    return Ok(token.clone());
                }
                Err(error)
            }
        }
    }

    async fn issue_token(&self) -> Result<InstallationTokenState> {
        let app_id = self.env.var("GITHUB_APP_ID")?.to_string();
        let installation_id = self.env.var("GITHUB_INSTALLATION_ID")?.to_string();
        let private_key = self.env.secret("GITHUB_APP_PRIVATE_KEY")?.to_string();
        let api_version = self
            .env
            .var("GITHUB_API_VERSION")
            .map(|value| value.to_string())
            .unwrap_or_else(|_| DEFAULT_GITHUB_API_VERSION.to_owned());
        let user_agent = self
            .env
            .var("GITHUB_USER_AGENT")
            .map(|value| value.to_string())
            .unwrap_or_else(|_| DEFAULT_USER_AGENT.to_owned());
        let jwt = create_app_jwt(&app_id, &private_key).await?;

        let url =
            format!("https://api.github.com/app/installations/{installation_id}/access_tokens");
        let headers = Headers::new();
        headers.set("Accept", GITHUB_JSON_ACCEPT)?;
        headers.set("User-Agent", &user_agent)?;
        headers.set("X-GitHub-Api-Version", &api_version)?;
        headers.set("Authorization", &format!("Bearer {jwt}"))?;
        let mut init = RequestInit::new();
        init.with_method(Method::Post).with_headers(headers);
        let request = Request::new_with_init(&url, &init)?;
        let mut response = Fetch::Request(request).send().await?;
        log_token_response(response.status_code(), response.headers());

        if !(200..300).contains(&response.status_code()) {
            return Err(format!(
                "GitHub installation token request failed: {}",
                response.status_code()
            )
            .into());
        }

        let body = response.json::<GithubInstallationTokenResponse>().await?;
        Ok(InstallationTokenState {
            token: body.token,
            expires_at_ms: parse_github_time_ms(&body.expires_at)
                .ok_or("GitHub installation token response had invalid expires_at")?,
        })
    }
}

#[derive(Serialize)]
struct TokenResponseBody {
    token: String,
    expires_at_ms: u64,
}

fn log_token_response(status: u16, headers: &Headers) {
    let request_id = header_or_default(headers, "x-github-request-id");
    let limit = header_or_default(headers, "x-ratelimit-limit");
    let remaining = header_or_default(headers, "x-ratelimit-remaining");
    let reset = header_or_default(headers, "x-ratelimit-reset");
    console_log!(
        "github_token_api status={} request_id={} rate_limit={} rate_remaining={} rate_reset={}",
        status,
        request_id,
        limit,
        remaining,
        reset
    );
}
