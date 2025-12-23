mod manifest;
mod registry;

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
        println!("Attempting to fetch metadata for dependency {} ({})", pkg_name, version_constraint);

        match client.get_package_metadata(pkg_name).await {
            Ok(versions) => {
                println!("Found {} released versions for {}", versions.len(), pkg_name);

                for v in versions.iter().take(3) {
                    println!("    - Version: {} (Normalized: {})", v.version, v.version_normalized);
                }
            }
            Err(e) => eprintln!("Error fetching package: {}", e),
        }
    } else {
        println!("No dependencies found in composer.json to test network fetch.");
    }

    Ok(())


}
