use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub const REQUIRED_DLLS: &[&str] = &[
    "VRC.Udon.Graph.dll",
    "VRC.Udon.VRCGraphModules.dll",
    "VRC.Udon.EditorBindings.dll",
    "VRC.Udon.VRCWrapperModules.dll",
    "VRC.Udon.VRCTypeResolverModules.dll",
];

#[derive(Debug, Clone)]
pub struct SdkTree {
    pub version: String,
    pub root: PathBuf,
}

pub fn discover_sdk_trees(root: impl AsRef<Path>) -> Result<Vec<SdkTree>> {
    let root = root.as_ref();
    let mut trees = Vec::new();
    for entry in fs::read_dir(root).with_context(|| format!("failed to read {}", root.display()))? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let Some(version) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if find_sdk_dlls(&path)?.is_empty() {
            continue;
        }
        trees.push(SdkTree {
            version: version.to_owned(),
            root: path,
        });
    }
    trees.sort_by(|a, b| a.version.cmp(&b.version));
    Ok(trees)
}

pub fn find_sdk_dlls(root: impl AsRef<Path>) -> Result<Vec<PathBuf>> {
    let root = root.as_ref();
    let mut found = Vec::new();
    collect_matching_dlls(root, &mut found)?;
    found.sort();
    Ok(found)
}

fn collect_matching_dlls(dir: &Path, found: &mut Vec<PathBuf>) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_matching_dlls(&path, found)?;
        } else if let Some(name) = path.file_name().and_then(|name| name.to_str())
            && REQUIRED_DLLS.contains(&name)
        {
            found.push(path);
        }
    }
    Ok(())
}
