mod manifest;
mod registry;
mod semver_compat;
mod installer;

use manifest::ComposerManifest;
use registry::{RegistryClient, PackageVersion};
use semver_compat::to_rust_version;
use anyhow::{Context, Result};
use std::collections::{VecDeque, HashSet};
use std::fs;

#[tokio::main]
async fn main() -> Result<()> {
    let path = "composer.json";
    let content = fs::read_to_string(path).context("Read failed")?;
    let manifest: ComposerManifest = serde_json::from_str(&content)?;
    let client = RegistryClient::new();

    // Queue: Stores (package_name, version_constraint)
    let mut queue: VecDeque<(String, String)> = VecDeque::new();

    // set: Stores package_names already processed
    let mut installed: HashSet<String> = HashSet::new();

    // prime queue with root dependencies
    for (name, constraint) in manifest.require {
        queue.push_back((name, constraint));
    }

    println!("Starting resolution for {} dependencies...", queue.len());

    // process queue
    while let Some((pkg_name, version_constraint)) = queue.pop_front() {
        if installed.contains(&pkg_name) {
            continue;
        }

        println!("Resolving: {} ({})", pkg_name, version_constraint);

        let best_package = match resolve_package(&client, &pkg_name, &version_constraint).await? {
            Some(pkg) => pkg,
            None => {
                eprintln!("Warning: Could not resolve {} {}", pkg_name, version_constraint);
                continue;
            }
        };

        println!(
            "Selected: {} v{}",
            best_package.name.as_deref().unwrap_or(&pkg_name),
            best_package.version
        );

        // Download
        if let Some(dist) = &best_package.dist {
            installer::install_package(&pkg_name, &best_package.version, &dist.url).await?;
        }

        // add pkg's dependencies to queue
        for (dep_name, dep_constraint) in best_package.require {
            if dep_name == "php" || dep_name.starts_with("ext-") {
                continue; // skip platform requirements
            }

            if !installed.contains(&dep_name) {
                queue.push_back((dep_name, dep_constraint));
            }
        }

        // mark installed
        installed.insert(pkg_name);
    }

    println!("All dependencies installed.");
    Ok(())
}

async fn resolve_package(
    client: &RegistryClient,
    pkg_name: &str,
    constraint: &str
) -> Result<Option<PackageVersion>> {
    let versions = client.get_package_metadata(pkg_name).await?;

    let mut compatible_versions: Vec<(&PackageVersion, semver::Version)> = versions.iter()
        .filter_map(|v| {
            let rust_v = to_rust_version(&v.version_normalized)?;
            if semver_compat::version_matches(constraint, &v.version_normalized) {
                Some((v, rust_v))
            } else {
                None
            }
        })
        .collect();

    if compatible_versions.is_empty() {
        return Ok(None);
    }

    compatible_versions.sort_by(|a, b| a.1.cmp(&b.1));

    // return clone of the best package
    Ok(compatible_versions.last().map(|(pkg, _)| (*pkg).clone()))
}