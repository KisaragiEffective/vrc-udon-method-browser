use std::cell::RefCell;

use vrc_udon_methods_worker_core::{
    DEFAULT_CACHE_TTL_SECONDS, DEFAULT_GITHUB_API_VERSION, DEFAULT_STALE_REVALIDATE_SECONDS,
    DEFAULT_USER_AGENT, validate_required_bindings,
};
use worker::Env;

thread_local! {
    static CONFIG_VALIDATION: RefCell<Option<std::result::Result<RuntimeConfig, Vec<String>>>> = const { RefCell::new(None) };
}

#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub api_version: String,
    pub user_agent: String,
    pub cache_ttl_seconds: u64,
    pub stale_revalidate_seconds: u64,
    pub log_body: bool,
}

pub fn runtime_config(env: &Env) -> std::result::Result<RuntimeConfig, Vec<String>> {
    CONFIG_VALIDATION.with(|cell| {
        if let Some(cached) = cell.borrow().as_ref() {
            return cached.clone();
        }

        let result = validate_runtime_config(env);
        *cell.borrow_mut() = Some(result.clone());
        result
    })
}

fn validate_runtime_config(env: &Env) -> std::result::Result<RuntimeConfig, Vec<String>> {
    if let Err(error) = validate_required_bindings(|binding| binding_exists(env, binding)) {
        return Err(error
            .missing
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>());
    }

    Ok(RuntimeConfig {
        api_version: env
            .var("GITHUB_API_VERSION")
            .map(|value| value.to_string())
            .unwrap_or_else(|_| DEFAULT_GITHUB_API_VERSION.to_owned()),
        user_agent: env
            .var("GITHUB_USER_AGENT")
            .map(|value| value.to_string())
            .unwrap_or_else(|_| DEFAULT_USER_AGENT.to_owned()),
        cache_ttl_seconds: env_u64(env, "GITHUB_CACHE_TTL_SECONDS", DEFAULT_CACHE_TTL_SECONDS),
        stale_revalidate_seconds: env_u64(
            env,
            "GITHUB_CACHE_STALE_SECONDS",
            DEFAULT_STALE_REVALIDATE_SECONDS,
        ),
        log_body: env_bool(env, "GITHUB_LOG_BODY", false),
    })
}

fn binding_exists(env: &Env, binding: &str) -> bool {
    match binding {
        "GITHUB_APP_ID" | "GITHUB_INSTALLATION_ID" => env
            .var(binding)
            .map(|value| binding_value_is_set(&value.to_string()))
            .unwrap_or(false),
        "GITHUB_APP_PRIVATE_KEY" => env
            .secret(binding)
            .map(|value| binding_value_is_set(&value.to_string()))
            .unwrap_or(false),
        "GITHUB_API_CACHE" => env.kv(binding).is_ok(),
        "GITHUB_TOKEN_BROKER" => env.durable_object(binding).is_ok(),
        _ => false,
    }
}

fn binding_value_is_set(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty() && !value.starts_with("replace-with-")
}

pub fn env_u64(env: &Env, binding: &str, default: u64) -> u64 {
    env.var(binding)
        .ok()
        .and_then(|value| value.to_string().parse::<u64>().ok())
        .unwrap_or(default)
}

fn env_bool(env: &Env, binding: &str, default: bool) -> bool {
    env.var(binding)
        .ok()
        .map(|value| matches!(value.to_string().as_str(), "1" | "true" | "TRUE" | "yes"))
        .unwrap_or(default)
}
