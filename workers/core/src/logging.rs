pub fn redact_log_body(body: &str, max_bytes: usize) -> String {
    const SECRET_KEYS: &[&str] = &[
        "authorization",
        "jwt",
        "token",
        "access_token",
        "private_key",
        "github_app_private_key",
    ];

    let mut value = match serde_json::from_str::<serde_json::Value>(body) {
        Ok(value) => value,
        Err(_) => {
            let truncated = truncate_utf8(body, max_bytes);
            return truncated.to_owned();
        }
    };
    redact_json_value(&mut value, SECRET_KEYS);
    let redacted = value.to_string();
    truncate_utf8(&redacted, max_bytes).to_owned()
}

fn redact_json_value(value: &mut serde_json::Value, secret_keys: &[&str]) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                if secret_keys
                    .iter()
                    .any(|secret_key| key.eq_ignore_ascii_case(secret_key))
                {
                    *value = serde_json::Value::String("[redacted]".to_owned());
                } else {
                    redact_json_value(value, secret_keys);
                }
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                redact_json_value(value, secret_keys);
            }
        }
        _ => {}
    }
}

fn truncate_utf8(value: &str, max_bytes: usize) -> &str {
    if value.len() <= max_bytes {
        return value;
    }

    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

#[cfg(test)]
mod tests {
    use crate::{MAX_LOGGED_BODY_BYTES, redact_log_body};

    #[test]
    fn redacts_sensitive_log_body() {
        let redacted = redact_log_body(
            r#"{"token":"secret","nested":{"Authorization":"Bearer x"},"ok":true}"#,
            MAX_LOGGED_BODY_BYTES,
        );

        assert!(redacted.contains("[redacted]"));
        assert!(!redacted.contains("secret"));
        assert!(!redacted.contains("Bearer x"));
    }
}
