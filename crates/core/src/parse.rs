use thiserror::Error;

use std::collections::BTreeMap;

use crate::model::{MethodRecord, UdonType};

pub type TypeNameMap = BTreeMap<String, String>;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ParseExternError {
    #[error("extern symbol is missing a declaring type separator: {0}")]
    MissingDeclaringType(String),
    #[error("extern symbol is missing a method signature separator: {0}")]
    MissingSignature(String),
    #[error("extern symbol has an empty declaring type or member name: {0}")]
    EmptyPart(String),
}

pub fn parse_extern_symbol(
    version: impl Into<String>,
    symbol: &str,
) -> Result<MethodRecord, ParseExternError> {
    parse_extern_symbol_with_types(version, symbol, &TypeNameMap::new())
}

pub fn parse_extern_symbol_with_types(
    version: impl Into<String>,
    symbol: &str,
    types: &TypeNameMap,
) -> Result<MethodRecord, ParseExternError> {
    let (declaring_type, signature) = symbol
        .split_once('.')
        .ok_or_else(|| ParseExternError::MissingDeclaringType(symbol.to_owned()))?;
    if declaring_type.is_empty() {
        return Err(ParseExternError::EmptyPart(symbol.to_owned()));
    }

    let signature = signature
        .strip_prefix("__")
        .ok_or_else(|| ParseExternError::MissingSignature(symbol.to_owned()))?;
    let parts: Vec<&str> = signature.split("__").collect();
    if parts.len() < 2 || parts[0].is_empty() {
        return Err(ParseExternError::MissingSignature(symbol.to_owned()));
    }

    let member_name = parts[0].to_owned();
    let return_part = parts.last().copied().unwrap_or("SystemVoid");
    let parameter_parts: Vec<String> = if parts.len() > 2 {
        parse_parameter_parts(&parts[1..parts.len() - 1].join("_"), types)
    } else {
        Vec::new()
    };
    let return_type = if return_part == "SystemVoid" {
        None
    } else {
        Some(parse_type_with_fqcn(return_part, types))
    };

    Ok(MethodRecord {
        version: version.into(),
        symbol: symbol.to_owned(),
        declaring_type: declaring_type.to_owned(),
        declaring_type_fqcn: lookup_type_fqcn(declaring_type, types),
        member_name,
        parameters: parameter_parts
            .iter()
            .map(|part| parse_type_with_fqcn(part.as_str(), types))
            .collect(),
        return_type,
    })
}

#[must_use]
pub fn parse_symbols(
    version: impl Into<String>,
    symbols: impl IntoIterator<Item = String>,
) -> Vec<MethodRecord> {
    parse_symbols_with_types(version, symbols, &TypeNameMap::new())
}

#[must_use]
pub fn parse_symbols_with_types(
    version: impl Into<String>,
    symbols: impl IntoIterator<Item = String>,
    types: &TypeNameMap,
) -> Vec<MethodRecord> {
    let version = version.into();
    let mut records: Vec<_> = symbols
        .into_iter()
        .filter_map(|symbol| parse_extern_symbol_with_types(version.clone(), &symbol, types).ok())
        .collect();
    records.sort_by(|a, b| a.symbol.cmp(&b.symbol));
    records.dedup_by(|a, b| a.symbol == b.symbol);
    records
}

#[must_use]
pub fn mangle_type_fqcn(fqcn: &str) -> String {
    fqcn.chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect()
}

fn parse_type_with_fqcn(input: &str, types: &TypeNameMap) -> UdonType {
    let ty = UdonType::parse(input);
    ty.with_fqcn(lookup_type_fqcn(input, types))
}

fn parse_parameter_parts(input: &str, types: &TypeNameMap) -> Vec<String> {
    if input.is_empty() {
        return Vec::new();
    }
    if types.is_empty() {
        return split_parameter_parts(input);
    }

    let atoms: Vec<&str> = input.split('_').filter(|part| !part.is_empty()).collect();
    let mut params = Vec::new();
    let mut index = 0;
    while index < atoms.len() {
        let mut found = None;
        for end in ((index + 1)..=atoms.len()).rev() {
            let candidate = atoms[index..end].join("_");
            if is_known_type_part(&candidate, types) {
                found = Some((candidate, end));
                break;
            }
        }

        if let Some((candidate, end)) = found {
            params.push(candidate);
            index = end;
        } else {
            params.push(atoms[index].to_owned());
            index += 1;
        }
    }
    params
}

fn split_parameter_parts(input: &str) -> Vec<String> {
    input
        .split('_')
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect()
}

fn is_known_type_part(input: &str, types: &TypeNameMap) -> bool {
    types.contains_key(normalize_type_key(input))
}

fn lookup_type_fqcn(input: &str, types: &TypeNameMap) -> Option<String> {
    types.get(normalize_type_key(input)).cloned()
}

fn normalize_type_key(input: &str) -> &str {
    let without_ref = input.strip_suffix("Ref").unwrap_or(input);
    if without_ref != "SystemArray"
        && let Some(base) = without_ref.strip_suffix("Array")
    {
        base
    } else {
        without_ref
    }
}

#[cfg(test)]
mod tests {
    use super::{
        TypeNameMap, mangle_type_fqcn, parse_extern_symbol, parse_extern_symbol_with_types,
        parse_symbols,
    };

    #[test]
    fn parses_void_method() {
        let record = parse_extern_symbol(
            "3.10.3",
            "SystemArray.__Clear__SystemArray_SystemInt32_SystemInt32__SystemVoid",
        )
        .unwrap();

        assert_eq!(record.declaring_type, "SystemArray");
        assert_eq!(record.member_name, "Clear");
        assert_eq!(record.parameters.len(), 3);
        assert_eq!(record.return_type, None);
    }

    #[test]
    fn parses_return_and_ref_types() {
        let record = parse_extern_symbol(
            "3.10.3",
            "SystemBoolean.__TryParse__SystemString_SystemBooleanRef__SystemBoolean",
        )
        .unwrap();

        assert_eq!(record.member_name, "TryParse");
        assert_eq!(record.parameters[1].mangled, "SystemBooleanRef");
        assert!(record.parameters[1].is_ref);
        assert_eq!(record.return_type.unwrap().mangled, "SystemBoolean");
    }

    #[test]
    fn system_array_is_not_treated_as_array_suffix() {
        let mut types = TypeNameMap::new();
        types.insert("SystemArray".to_owned(), "System.Array".to_owned());
        types.insert(
            "VRCDynamicsContactEnterInfo".to_owned(),
            "VRC.Dynamics.ContactEnterInfo".to_owned(),
        );

        let record = parse_extern_symbol_with_types(
            "3.10.3",
            "VRCDynamicsContactEnterInfoArray.__CopyTo__SystemArray_SystemInt32__SystemVoid",
            &types,
        )
        .unwrap();

        assert_eq!(
            record.declaring_type_fqcn.as_deref(),
            Some("VRC.Dynamics.ContactEnterInfo")
        );
        assert!(!record.parameters[0].is_array);
        assert_eq!(record.parameters[0].fqcn.as_deref(), Some("System.Array"));
    }

    #[test]
    fn restores_fqcn_from_type_map() {
        let mut types = TypeNameMap::new();
        types.insert("SystemBoolean".to_owned(), "System.Boolean".to_owned());
        types.insert("SystemString".to_owned(), "System.String".to_owned());

        let record = parse_extern_symbol_with_types(
            "3.10.3",
            "SystemBoolean.__TryParse__SystemString_SystemBooleanRef__SystemBoolean",
            &types,
        )
        .unwrap();

        assert_eq!(
            record.declaring_type_fqcn,
            Some("System.Boolean".to_owned())
        );
        assert_eq!(record.parameters[0].fqcn, Some("System.String".to_owned()));
        assert_eq!(
            record.return_type.as_ref().and_then(|ty| ty.fqcn.clone()),
            Some("System.Boolean".to_owned())
        );
    }

    #[test]
    fn keeps_underscores_inside_known_type_names() {
        let mut types = TypeNameMap::new();
        types.insert(
            "UnityEngineVector3".to_owned(),
            "UnityEngine.Vector3".to_owned(),
        );
        types.insert(
            "UnityEngineQuaternion".to_owned(),
            "UnityEngine.Quaternion".to_owned(),
        );
        types.insert(
            "VRCSDKBaseVRC_SceneDescriptorSpawnOrientation".to_owned(),
            "VRC.SDKBase.VRC_SceneDescriptor/SpawnOrientation".to_owned(),
        );

        let record = parse_extern_symbol_with_types(
            "3.10.3",
            "VRCSDKBaseVRCPlayerApi.__TeleportTo__UnityEngineVector3_UnityEngineQuaternion_VRCSDKBaseVRC_SceneDescriptorSpawnOrientation__SystemVoid",
            &types,
        )
        .unwrap();

        assert_eq!(record.parameters.len(), 3);
        assert_eq!(
            record.parameters[2].mangled,
            "VRCSDKBaseVRC_SceneDescriptorSpawnOrientation"
        );
        assert_eq!(
            record.parameters[2].fqcn.as_deref(),
            Some("VRC.SDKBase.VRC_SceneDescriptor/SpawnOrientation")
        );
    }

    #[test]
    fn mangle_type_fqcn_matches_udon_type_names() {
        assert_eq!(mangle_type_fqcn("System.String"), "SystemString");
        assert_eq!(
            mangle_type_fqcn("UnityEngine.Vector3"),
            "UnityEngineVector3"
        );
        assert_eq!(
            mangle_type_fqcn("VRC.SDKBase.VRC_Pickup"),
            "VRCSDKBaseVRC_Pickup"
        );
        assert_eq!(
            mangle_type_fqcn("Namespace.Outer/Inner"),
            "NamespaceOuterInner"
        );
    }

    #[test]
    fn rejects_non_extern_strings() {
        assert!(parse_extern_symbol("x", "SystemArray.Clone").is_err());
        assert!(parse_extern_symbol("x", "__Clone__SystemObject").is_err());
    }

    #[test]
    fn sorts_and_deduplicates_symbols() {
        let records = parse_symbols(
            "v",
            [
                "B.__Y__SystemVoid".to_owned(),
                "A.__X__SystemVoid".to_owned(),
                "A.__X__SystemVoid".to_owned(),
            ],
        );

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].symbol, "A.__X__SystemVoid");
    }
}
