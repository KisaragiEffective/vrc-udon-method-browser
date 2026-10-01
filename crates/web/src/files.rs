use gloo_net::http::Request;

pub async fn download_bytes(url: &str) -> Result<Vec<u8>, String> {
    Request::get(url)
        .send()
        .await
        .map_err(|error| error.to_string())?
        .binary()
        .await
        .map_err(|error| error.to_string())
}

#[must_use]
pub fn looks_like_zip(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK\x03\x04")
        || bytes.starts_with(b"PK\x05\x06")
        || bytes.starts_with(b"PK\x07\x08")
}
