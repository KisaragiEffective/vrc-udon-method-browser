use thiserror::Error;

pub const REQUIRED_BINDINGS: &[&str] = &[
    "GITHUB_APP_ID",
    "GITHUB_INSTALLATION_ID",
    "GITHUB_APP_PRIVATE_KEY",
    "GITHUB_API_CACHE",
    "GITHUB_TOKEN_BROKER",
];

#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("ConfigurationError: missing required bindings: {missing:?}")]
pub struct ConfigurationError {
    pub missing: Vec<&'static str>,
}

pub fn validate_required_bindings(
    mut exists: impl FnMut(&'static str) -> bool,
) -> Result<(), ConfigurationError> {
    let missing = REQUIRED_BINDINGS
        .iter()
        .copied()
        .filter(|binding| !exists(binding))
        .collect::<Vec<_>>();

    if missing.is_empty() {
        Ok(())
    } else {
        Err(ConfigurationError { missing })
    }
}

#[cfg(test)]
mod tests {
    use super::validate_required_bindings;

    #[test]
    fn reports_missing_required_bindings() {
        let err =
            validate_required_bindings(|binding| binding != "GITHUB_APP_PRIVATE_KEY").unwrap_err();

        assert_eq!(err.missing, vec!["GITHUB_APP_PRIVATE_KEY"]);
        assert!(err.to_string().contains("ConfigurationError"));
    }
}
