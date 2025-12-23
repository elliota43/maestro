mod manifest;
mod registry;
mod semver_compat;

use manifest::ComposerManifest;
use registry::RegistryClient;
use anyhow::{Context, Result};
use std::fs;

#[tokio::main]
async fn main() -> Result<()> {
    let path = "composer.json";
    println!("Looking for {}...", path);

    let content = fs::read_to_string(path)
        .with_context(|| format!("Could not read file `{}`", path))?;

    let manifest: ComposerManifest = serde_json::from_str(&content)
        .context("composer.json has invalid JSON syntax")?;

    println!("✅ Successfully parsed manifest!");

    let client = RegistryClient::new();

    if let Some((pkg_name, version_constraint)) = manifest.require.iter().next() {
        println!("Resolving: {} ({})", pkg_name, version_constraint);

        let versions = client.get_package_metadata(pkg_name).await?;

        println!("Filtering {} versions against contraint '{}'...", versions.len(), version_constraint);

        let mut valid_versions: Vec<String> = versions.iter()
            .filter(|v| {
                semver_compat::version_matches(version_constraint, &v.version_normalized)
            })
            .map(|v| v.version.clone())
            .collect();

        valid_versions.reverse();

        if valid_versions.is_empty() {
            println!("No matching versions found!");
        } else{
            println!("Found {} matching versions:", valid_versions.len());
            for v in valid_versions.iter().take(5) {
                println!("    -{}", v);
            }
        }
    }
    Ok(())


}
