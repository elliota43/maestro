mod manifest;
use manifest::ComposerManifest;
use anyhow::{Context, Result};
use std::fs;

fn main() -> Result<()> {
    let path = "composer.json";
    println!("Looking for {}...", path);

    let content = fs::read_to_string(path)
        .with_context(|| format!("Could not read file `{}`", path))?;

    let manifest: ComposerManifest = serde_json::from_str(&content)
        .context("composer.json has invalid JSON syntax")?;

    println!("✅ Successfully parsed manifest!");

    if let Some(name) = manifest.name {
        println!("📦 Package: {}", name);
    }

    println!("Found {} production dependencies:", manifest.require.len());
    for (pkg, version) in manifest.require {
        println!("- {}: {}", pkg, version);
    }

    Ok(())


}
