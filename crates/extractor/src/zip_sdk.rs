use anyhow::{Context, Result, anyhow};
use std::io::{Cursor, Read};

use crate::discovery::REQUIRED_DLLS;

pub fn extract_required_dlls_from_zip(zip_bytes: &[u8]) -> Result<Vec<Vec<u8>>> {
    let reader = Cursor::new(zip_bytes);
    let mut archive = zip::ZipArchive::new(reader).context("failed to read SDK zip")?;
    let mut assemblies = Vec::new();

    for index in 0..archive.len() {
        let mut file = archive.by_index(index)?;
        if !REQUIRED_DLLS
            .iter()
            .any(|required| file.name().ends_with(required))
        {
            continue;
        }

        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        assemblies.push(bytes);
    }

    if assemblies.is_empty() {
        return Err(anyhow!("SDK zip did not contain required Udon DLLs"));
    }
    Ok(assemblies)
}
