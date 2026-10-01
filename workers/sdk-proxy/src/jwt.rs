use base64::{Engine as _, engine::general_purpose};
use js_sys::{Array, Object, Reflect, Uint8Array};
use serde::Serialize;
use vrc_udon_methods_worker_core::{DEFAULT_JWT_BACKDATE_SECONDS, DEFAULT_JWT_LIFETIME_SECONDS};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use worker::Result;

use crate::time::now_ms;

#[derive(Serialize)]
struct GithubJwtClaims<'a> {
    iat: u64,
    exp: u64,
    iss: &'a str,
}

pub async fn create_app_jwt(app_id: &str, private_key_pem: &str) -> Result<String> {
    let now_seconds = now_ms() / 1000;
    let header = serde_json::json!({ "alg": "RS256", "typ": "JWT" });
    let claims = GithubJwtClaims {
        iat: now_seconds.saturating_sub(DEFAULT_JWT_BACKDATE_SECONDS),
        exp: now_seconds + DEFAULT_JWT_LIFETIME_SECONDS,
        iss: app_id,
    };
    let signing_input = format!("{}.{}", base64url_json(&header)?, base64url_json(&claims)?);
    let signature = sign_rs256(private_key_pem, signing_input.as_bytes()).await?;
    Ok(format!(
        "{signing_input}.{}",
        general_purpose::URL_SAFE_NO_PAD.encode(signature)
    ))
}

fn base64url_json<T: Serialize>(value: &T) -> Result<String> {
    Ok(general_purpose::URL_SAFE_NO_PAD.encode(serde_json::to_vec(value)?))
}

async fn sign_rs256(private_key_pem: &str, data: &[u8]) -> Result<Vec<u8>> {
    let key_der = pem_to_der(private_key_pem)?;
    let global = js_sys::global();
    let crypto = Reflect::get(&global, &JsValue::from_str("crypto"))?;
    let subtle = Reflect::get(&crypto, &JsValue::from_str("subtle"))?;

    let algorithm = Object::new();
    Reflect::set(
        &algorithm,
        &JsValue::from_str("name"),
        &JsValue::from_str("RSASSA-PKCS1-v1_5"),
    )?;
    Reflect::set(
        &algorithm,
        &JsValue::from_str("hash"),
        &JsValue::from_str("SHA-256"),
    )?;

    let usages = Array::new();
    usages.push(&JsValue::from_str("sign"));
    let key_bytes = Uint8Array::from(key_der.as_slice());
    let import_key =
        Reflect::get(&subtle, &JsValue::from_str("importKey"))?.dyn_into::<js_sys::Function>()?;
    let promise = import_key.call5(
        &subtle,
        &JsValue::from_str("pkcs8"),
        key_bytes.as_ref(),
        &algorithm,
        &JsValue::FALSE,
        &usages,
    )?;
    let crypto_key = JsFuture::from(js_sys::Promise::from(promise)).await?;

    let data_bytes = Uint8Array::from(data);
    let sign = Reflect::get(&subtle, &JsValue::from_str("sign"))?.dyn_into::<js_sys::Function>()?;
    let promise = sign.call3(&subtle, &algorithm, &crypto_key, data_bytes.as_ref())?;
    let signature = JsFuture::from(js_sys::Promise::from(promise)).await?;
    Ok(Uint8Array::new(&signature).to_vec())
}

fn pem_to_der(private_key_pem: &str) -> Result<Vec<u8>> {
    let normalized = private_key_pem.replace("\\n", "\n");
    let body = normalized
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .map(str::trim)
        .collect::<String>();
    general_purpose::STANDARD
        .decode(body)
        .map_err(|error| format!("invalid GITHUB_APP_PRIVATE_KEY PEM: {error}").into())
}
