use anyhow::{Context, Result, anyhow};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use vrc_udon_methods_core::{MethodRecord, TypeNameMap, parse_symbols_with_types};

use crate::assembly::{exposed_type_map_from_bytes, validate_dotnet_assembly};
use crate::discovery::find_sdk_dlls;
use crate::strings::{scan_ascii_strings, scan_utf16le_strings};

pub fn extract_tree(
    version: impl Into<String>,
    tree_root: impl AsRef<Path>,
) -> Result<Vec<MethodRecord>> {
    let version = version.into();
    let dlls = find_sdk_dlls(tree_root)?;
    if dlls.is_empty() {
        return Err(anyhow!("no Udon SDK DLLs found"));
    }

    let mut symbols = BTreeSet::new();
    let mut types = TypeNameMap::new();
    for dll in dlls {
        let bytes = fs::read(&dll).with_context(|| format!("failed to read {}", dll.display()))?;
        if let Err(error) = validate_dotnet_assembly(&bytes, &dll) {
            eprintln!("warning: {error:#}");
        }
        if let Err(error) = merge_exposed_types(&mut types, &bytes) {
            eprintln!(
                "warning: failed to read exposed types from {}: {error:#}",
                dll.display()
            );
        }
        symbols.extend(extract_mangled_symbols_from_bytes(&bytes));
    }

    records_or_error(version, symbols, &types)
}

pub fn extract_bytes(
    version: impl Into<String>,
    assemblies: &[Vec<u8>],
) -> Result<Vec<MethodRecord>> {
    let version = version.into();
    let mut symbols = BTreeSet::new();
    let mut types = TypeNameMap::new();
    for bytes in assemblies {
        merge_exposed_types(&mut types, bytes)?;
        symbols.extend(extract_mangled_symbols_from_bytes(bytes));
    }
    records_or_error(version, symbols, &types)
}

fn records_or_error(
    version: String,
    symbols: impl IntoIterator<Item = String>,
    types: &TypeNameMap,
) -> Result<Vec<MethodRecord>> {
    let records = parse_symbols_with_types(version, symbols, types);
    if records.is_empty() {
        return Err(anyhow!("no mangled extern symbols found"));
    }
    Ok(records)
}

fn merge_exposed_types(types: &mut TypeNameMap, bytes: &[u8]) -> Result<()> {
    for (mangled, fqcn) in exposed_type_map_from_bytes(bytes)? {
        types.entry(mangled).or_insert(fqcn);
    }
    Ok(())
}

pub fn extract_mangled_symbols_from_bytes(bytes: &[u8]) -> BTreeSet<String> {
    let mut symbols = BTreeSet::new();
    symbols.extend(
        scan_ascii_strings(bytes)
            .into_iter()
            .filter(|symbol| looks_like_extern_symbol(symbol)),
    );
    symbols.extend(
        scan_utf16le_strings(bytes)
            .into_iter()
            .filter(|symbol| looks_like_extern_symbol(symbol)),
    );
    symbols
}

#[must_use]
pub fn looks_like_extern_symbol(value: &str) -> bool {
    if value.len() < 12 || value.len() > 512 {
        return false;
    }
    if !value.contains(".__") || !value.contains("__") {
        return false;
    }
    value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_'))
}

#[cfg(test)]
mod tests {
    use super::extract_mangled_symbols_from_bytes;

    #[test]
    fn extracts_utf16le_symbols_from_bytes() {
        let symbol = "SystemBoolean.__TryParse__SystemString_SystemBooleanRef__SystemBoolean";
        let mut bytes = Vec::new();
        for unit in symbol.encode_utf16() {
            bytes.extend(unit.to_le_bytes());
        }

        let symbols = extract_mangled_symbols_from_bytes(&bytes);
        assert!(symbols.contains(symbol));
    }

    #[test]
    fn ignores_non_extern_strings() {
        let symbols = extract_mangled_symbols_from_bytes(b"SystemArray.Clone\0<foo>b__0\0");
        assert!(symbols.is_empty());
    }
}
