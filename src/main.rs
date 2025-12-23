mod manifest;
mod registry;
mod semver_compat;
mod installer;

use manifest::ComposerManifest;
use registry::RegistryClient;
use semver_compat::to_rust_version;
use anyhow::{Context, Result};
use std::fs;

#[tokio::main]
async fn main() -> Result<()> {
    let path = "composer.json";
    let content = fs::read_to_string(path).context("Read failed")?;
    let manifest: ComposerManifest = serde_json::from_str(&content)?;
    let client = RegistryClient::new();

    if let Some((pkg_name, version_constraint)) = manifest.require.iter().next() {
        println!("Resolving: {} ({})", pkg_name, version_constraint);

        let versions = client.get_package_metadata(pkg_name).await?;

        let mut compatible_versions: Vec<(&registry::PackageVersion, semver::Version)> = versions.iter()
            .filter_map(|v| {
                let rust_v = to_rust_version(&v.version_normalized)?;

                if semver_compat::version_matches(version_constraint, &v.version_normalized) {
                    Some((v, rust_v))
                } else {
                    None
                }
            })
            .collect();

        if compatible_versions.is_empty() {
            println!("No matching versions found.");
            return Ok(());
        }

        compatible_versions.sort_by(|a, b| a.1.cmp(&b.1));

        let (best_package, _best_semver) = compatible_versions.last().unwrap();

        println!(
            "Selected: {} v{}",
            best_package.name.as_deref().unwrap_or(pkg_name),
            best_package.version
        );

        if let Some(dist) = &best_package.dist {
            installer::install_package(pkg_name, &best_package.version, &dist.url).await?;
        } else {
            println!("Selected package has no download URL.");
        }

    } else {
        println!("No dependencies found in composer.json");
    }

    Ok(())
}