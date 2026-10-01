#![deny(clippy::all)]
#![warn(clippy::nursery)]

mod assembly;
mod discovery;
mod extraction;
mod strings;
mod zip_sdk;

pub use discovery::{SdkTree, discover_sdk_trees, find_sdk_dlls};
pub use extraction::{
    extract_bytes, extract_mangled_symbols_from_bytes, extract_tree, looks_like_extern_symbol,
};
pub use zip_sdk::extract_required_dlls_from_zip;

use anyhow::Result;
use vrc_udon_methods_core::MethodRecord;

pub fn extract_zip_bytes(
    version: impl Into<String>,
    zip_bytes: &[u8],
) -> Result<Vec<MethodRecord>> {
    let assemblies = extract_required_dlls_from_zip(zip_bytes)?;
    extract_bytes(version, &assemblies)
}
