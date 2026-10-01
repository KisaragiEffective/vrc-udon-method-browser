use serde::Serialize;
use worker::{Env, Headers, Response, Result};

#[derive(Serialize)]
pub struct ErrorBody {
    error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    missing: Option<Vec<String>>,
}

impl ErrorBody {
    pub fn new(error: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            missing: None,
        }
    }

    fn configuration(missing: Vec<String>) -> Self {
        Self {
            error: format!(
                "ConfigurationError: missing required bindings: {}",
                missing.join(", ")
            ),
            missing: Some(missing),
        }
    }
}

pub fn configuration_error_response(missing: Vec<String>, env: &Env) -> Result<Response> {
    json_response(&ErrorBody::configuration(missing), 503, env)
}

pub fn json_response<T: Serialize>(value: &T, status: u16, env: &Env) -> Result<Response> {
    let mut response = Response::from_json(value)?.with_status(status);
    set_cors_headers(response.headers_mut(), env)?;
    response
        .headers_mut()
        .set("Content-Type", "application/json; charset=utf-8")?;
    Ok(response)
}

pub fn empty_response(status: u16, env: &Env) -> Result<Response> {
    let mut response = Response::empty()?.with_status(status);
    set_cors_headers(response.headers_mut(), env)?;
    Ok(response)
}

pub fn set_cors_headers(headers: &Headers, env: &Env) -> Result<()> {
    let origin = env
        .var("ALLOWED_ORIGIN")
        .map(|value| value.to_string())
        .unwrap_or_else(|_| "*".to_owned());
    headers.set("Access-Control-Allow-Origin", &origin)?;
    headers.set("Access-Control-Allow-Methods", "GET, OPTIONS")?;
    headers.set("Access-Control-Allow-Headers", "Content-Type")?;
    Ok(())
}
