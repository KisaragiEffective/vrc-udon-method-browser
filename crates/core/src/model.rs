use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MethodRecord {
    pub version: String,
    pub symbol: String,
    pub declaring_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declaring_type_fqcn: Option<String>,
    pub member_name: String,
    pub parameters: Vec<UdonType>,
    pub return_type: Option<UdonType>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UdonType {
    pub mangled: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fqcn: Option<String>,
    pub is_ref: bool,
    pub is_array: bool,
}

impl UdonType {
    #[must_use]
    pub fn parse(input: &str) -> Self {
        let is_ref = input.ends_with("Ref");
        let without_ref = input.strip_suffix("Ref").unwrap_or(input);
        let is_array = without_ref != "SystemArray" && without_ref.ends_with("Array");

        Self {
            mangled: input.to_owned(),
            fqcn: None,
            is_ref,
            is_array,
        }
    }

    #[must_use]
    pub fn with_fqcn(mut self, fqcn: Option<String>) -> Self {
        self.fqcn = fqcn;
        self
    }

    #[must_use]
    pub fn display_name(&self) -> String {
        let mut base = self.fqcn.clone().unwrap_or_else(|| {
            self.mangled
                .strip_suffix("Ref")
                .unwrap_or(&self.mangled)
                .to_owned()
        });
        if self.is_array && !base.ends_with("[]") {
            if let Some(stripped) = base.strip_suffix("Array") {
                base = format!("{stripped}[]");
            } else {
                base = format!("{base}[]");
            }
        }
        if self.is_ref {
            format!("{base}&")
        } else {
            base
        }
    }
}

#[cfg(test)]
mod tests {
    use super::UdonType;

    #[test]
    fn parses_arrays() {
        let ty = UdonType::parse("UnityEngineVector3Array");
        assert!(ty.is_array);
        assert_eq!(ty.display_name(), "UnityEngineVector3[]");
    }

    #[test]
    fn display_prefers_fqcn() {
        let ty = UdonType::parse("UnityEngineVector3Array")
            .with_fqcn(Some("UnityEngine.Vector3".to_owned()));
        assert_eq!(ty.display_name(), "UnityEngine.Vector3[]");
    }
}
