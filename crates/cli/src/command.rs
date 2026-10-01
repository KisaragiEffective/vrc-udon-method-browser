use anyhow::{Context, Result, bail};
use std::path::PathBuf;
use vrc_udon_methods_extractor::{discover_sdk_trees, extract_tree};

pub fn run(mut args: impl Iterator<Item = String>) -> Result<()> {
    match args.next().as_deref() {
        Some("extract") => extract_command(&mut args),
        Some("check-trees") => check_trees_command(&mut args),
        _ => bail!(
            "usage: vrc-udon-methods extract <version> <sdk-tree> | check-trees [world-sdk/trees]"
        ),
    }
}

fn extract_command(args: &mut impl Iterator<Item = String>) -> Result<()> {
    let version = args.next().context("missing version")?;
    let tree = PathBuf::from(args.next().context("missing SDK tree path")?);
    let records = extract_tree(version, tree)?;
    println!("{}", serde_json::to_string_pretty(&records)?);
    Ok(())
}

fn check_trees_command(args: &mut impl Iterator<Item = String>) -> Result<()> {
    let root = PathBuf::from(args.next().unwrap_or_else(|| "world-sdk/trees".to_owned()));
    let trees = discover_sdk_trees(root)?;
    if trees.is_empty() {
        bail!("no SDK trees found");
    }
    for tree in trees {
        let records = extract_tree(tree.version.clone(), &tree.root)
            .with_context(|| format!("failed to extract {}", tree.version))?;
        println!("{}\t{}", tree.version, records.len());
    }
    Ok(())
}
