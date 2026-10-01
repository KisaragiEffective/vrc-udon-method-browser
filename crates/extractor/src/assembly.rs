use anyhow::Result;
use std::path::Path;
use vrc_udon_methods_core::{TypeNameMap, mangle_type_fqcn};

pub fn validate_dotnet_assembly(bytes: &[u8], path: &Path) -> Result<()> {
    validate_dotnet_assembly_impl(bytes, path)
}

pub fn exposed_type_map_from_bytes(bytes: &[u8]) -> Result<TypeNameMap> {
    exposed_type_map_from_bytes_impl(bytes)
}

#[cfg(not(target_arch = "wasm32"))]
fn validate_dotnet_assembly_impl(bytes: &[u8], path: &Path) -> Result<()> {
    use anyhow::Context as _;

    let _assembly = dotscope::CilAssemblyView::from_mem_with_validation(
        bytes.to_vec(),
        dotscope::ValidationConfig::minimal(),
    )
    .with_context(|| format!("dotscope failed to load {}", path.display()))?;
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn exposed_type_map_from_bytes_impl(bytes: &[u8]) -> Result<TypeNameMap> {
    let assembly = dotscope::CilObject::from_mem_with_validation(
        bytes.to_vec(),
        dotscope::ValidationConfig::minimal(),
    )?;
    let mut types = builtin_type_map();

    let has_extern_wrappers = assembly
        .types()
        .all_types()
        .iter()
        .any(|ty| is_udon_extern_wrapper(&ty.fullname()));
    if has_extern_wrappers {
        for method_spec in assembly.method_specs().iter() {
            for (_, type_ref) in method_spec.value().generic_args.iter() {
                let Some(type_arg) = type_ref.upgrade() else {
                    continue;
                };
                insert_type_name(&mut types, &format_cil_type_name(&type_arg));
            }
        }
    }
    insert_vrc_type_resolver_types(&assembly, &mut types)?;
    insert_node_registry_types(&assembly, &mut types)?;

    Ok(types)
}

#[cfg(not(target_arch = "wasm32"))]
fn is_udon_extern_wrapper(fullname: &str) -> bool {
    fullname
        .strip_prefix("VRC.Udon.Wrapper.Modules.Extern")
        .is_some_and(|name| !name.is_empty() && !name.contains("<>"))
}

#[cfg(not(target_arch = "wasm32"))]
fn format_cil_type_name(ty: &dotscope::metadata::typesystem::CilType) -> String {
    let mut fqcn = ty.fullname();
    if let Some((base, _arity)) = fqcn.rsplit_once('`') {
        fqcn = base.to_owned();
    }

    let generic_args = generic_arg_names(ty);
    if generic_args.is_empty() {
        fqcn
    } else {
        format!("{fqcn}<{}>", generic_args.join(", "))
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn generic_arg_names(ty: &dotscope::metadata::typesystem::CilType) -> Vec<String> {
    let mut args = Vec::new();
    for (_, method_spec) in ty.generic_args.iter() {
        for (_, type_ref) in method_spec.generic_args.iter() {
            if let Some(type_arg) = type_ref.upgrade() {
                args.push(format_cil_type_name(&type_arg));
            }
        }
    }
    args
}

#[cfg(not(target_arch = "wasm32"))]
fn insert_node_registry_types(
    assembly: &dotscope::CilObject,
    types: &mut TypeNameMap,
) -> Result<()> {
    use dotscope::assembly::Operand;

    for registry_type in assembly
        .types()
        .all_types()
        .into_iter()
        .filter(|ty| is_udon_node_registry(&ty.fullname()))
    {
        let Some(cctor) = registry_type
            .methods()
            .find(|method| method.name == ".cctor")
        else {
            continue;
        };
        let Some(instructions) = decode_method_instructions(assembly, &cctor)? else {
            continue;
        };

        let mut pending_symbol: Option<String> = None;
        for instruction in instructions {
            match (instruction.mnemonic, &instruction.operand) {
                ("ldstr", Operand::Token(token)) => {
                    let literal = resolver_user_string(assembly, token);
                    if literal.as_deref().is_some_and(|value| {
                        value.contains(".__") || value.starts_with("Variable_")
                    }) {
                        pending_symbol = literal;
                    }
                }
                ("ldtoken", Operand::Token(token)) => {
                    let Some(fqcn) = resolve_type_token_name(assembly, token) else {
                        continue;
                    };
                    insert_type_name(types, &fqcn);

                    if let Some(symbol) = pending_symbol.as_deref() {
                        insert_placeholder_type(types, symbol, &fqcn);
                    }
                }
                _ => {}
            }
        }
    }

    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn is_udon_node_registry(fullname: &str) -> bool {
    fullname
        .strip_prefix("VRC.Udon.Graph.NodeRegistries.")
        .is_some_and(|name| name.ends_with("NodeRegistry") && !name.contains("<>"))
}

#[cfg(not(target_arch = "wasm32"))]
fn insert_vrc_type_resolver_types(
    assembly: &dotscope::CilObject,
    types: &mut TypeNameMap,
) -> Result<()> {
    use dotscope::assembly::Operand;

    let Some(resolver_type) = assembly
        .types()
        .all_types()
        .into_iter()
        .find(|ty| ty.fullname() == "VRC.Udon.VRCTypeResolverModules.VRCTypeResolver")
    else {
        return Ok(());
    };
    let Some(cctor) = resolver_type
        .methods()
        .find(|method| method.name == ".cctor")
    else {
        return Ok(());
    };
    let Some(instructions) = decode_method_instructions(assembly, &cctor)? else {
        return Ok(());
    };

    let mut pending_key = None;
    for instruction in instructions {
        match (instruction.mnemonic, &instruction.operand) {
            ("ldstr", Operand::Token(token)) => {
                pending_key = resolver_user_string(assembly, token);
            }
            ("ldtoken", Operand::Token(token)) => {
                if let Some(key) = pending_key.take()
                    && let Some(fqcn) = resolve_type_token_name(assembly, token)
                {
                    insert_type_entry(types, &key, &fqcn);
                }
            }
            _ => {}
        }
    }

    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn decode_method_instructions(
    assembly: &dotscope::CilObject,
    method: &dotscope::metadata::method::Method,
) -> Result<Option<Vec<dotscope::assembly::Instruction>>> {
    let Some(rva) = method.rva else {
        return Ok(None);
    };
    let Some(body) = method.body.get() else {
        return Ok(None);
    };

    let file = assembly.file();
    let body_offset = file.rva_to_offset(rva as usize)?;
    let code_offset = body_offset + body.size_header;
    let code_end = code_offset + body.size_code;
    let Some(code) = file.data().get(code_offset..code_end) else {
        return Ok(None);
    };

    let mut parser = dotscope::Parser::new(code);
    Ok(Some(dotscope::assembly::decode_stream(
        &mut parser,
        u64::from(rva) + u64::try_from(body.size_header).unwrap_or(0),
    )?))
}

#[cfg(not(target_arch = "wasm32"))]
fn resolver_user_string(
    assembly: &dotscope::CilObject,
    token: &dotscope::metadata::token::Token,
) -> Option<String> {
    let userstrings = assembly.userstrings()?;
    userstrings
        .get(token.value() as usize)
        .or_else(|_| userstrings.get(token.row() as usize))
        .ok()
        .map(|literal| literal.to_string_lossy())
}

#[cfg(not(target_arch = "wasm32"))]
fn resolve_type_token_name(
    assembly: &dotscope::CilObject,
    token: &dotscope::metadata::token::Token,
) -> Option<String> {
    assembly
        .types()
        .get(token)
        .map(|ty| format_cil_type_name(&ty))
}

fn insert_placeholder_type(types: &mut TypeNameMap, symbol: &str, fqcn: &str) {
    if symbol.contains("ListT")
        && generic_type_base(fqcn) == Some("System.Collections.Generic.List")
    {
        insert_type_entry(types, "ListT", fqcn);
    }
    if symbol.contains("__T") && fqcn == "System.Object" {
        insert_type_entry(types, "T", fqcn);
    }
    if symbol.contains("TArray") && fqcn == "System.Object[]" {
        insert_type_entry(types, "TArray", fqcn);
    }
}

fn generic_type_base(fqcn: &str) -> Option<&str> {
    fqcn.split_once('<').map(|(base, _)| base)
}

fn insert_type_entry(types: &mut TypeNameMap, mangled: &str, fqcn: &str) {
    let mangled = normalize_type_key(mangled);
    let fqcn = fqcn
        .strip_suffix("[]")
        .or_else(|| fqcn.strip_suffix("&"))
        .unwrap_or(fqcn);
    if mangled.is_empty() || fqcn.is_empty() || fqcn == "<Module>" {
        return;
    }
    types
        .entry(mangled.to_owned())
        .or_insert_with(|| fqcn.to_owned());
}

fn insert_type_name(types: &mut TypeNameMap, fqcn: &str) {
    let fqcn = fqcn.trim();
    if fqcn.is_empty() || fqcn == "<Module>" {
        return;
    }

    let base = fqcn
        .strip_suffix("[]")
        .or_else(|| fqcn.strip_suffix("&"))
        .unwrap_or(fqcn);
    insert_type_entry(types, &mangle_type_fqcn(base), base);
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

#[cfg(target_arch = "wasm32")]
fn validate_dotnet_assembly_impl(_bytes: &[u8], _path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn exposed_type_map_from_bytes_impl(_bytes: &[u8]) -> Result<TypeNameMap> {
    Ok(builtin_type_map())
}

fn builtin_type_map() -> TypeNameMap {
    [
        ("SystemBoolean", "System.Boolean"),
        ("SystemByte", "System.Byte"),
        ("SystemSByte", "System.SByte"),
        ("SystemChar", "System.Char"),
        ("SystemDecimal", "System.Decimal"),
        ("SystemDouble", "System.Double"),
        ("SystemSingle", "System.Single"),
        ("SystemInt16", "System.Int16"),
        ("SystemInt32", "System.Int32"),
        ("SystemInt64", "System.Int64"),
        ("SystemUInt16", "System.UInt16"),
        ("SystemUInt32", "System.UInt32"),
        ("SystemUInt64", "System.UInt64"),
        ("SystemString", "System.String"),
        ("SystemObject", "System.Object"),
        ("SystemArray", "System.Array"),
        ("SystemVoid", "System.Void"),
    ]
    .into_iter()
    .map(|(mangled, fqcn)| (mangled.to_owned(), fqcn.to_owned()))
    .collect()
}
