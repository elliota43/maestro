mod manifest;
mod registry;
mod semver_compat;
mod installer;
mod generator;
mod cache;
mod lock; // <--- Register module

use manifest::ComposerManifest;
use registry::{RegistryClient, PackageVersion};
use semver_compat::to_rust_version;
use anyhow::{Context, Result};
use std::collections::{VecDeque, HashSet};
use std::fs;
use std::path::Path; // Need Path to check existence
use tokio::task::JoinSet;
use colored::Colorize;

#[tokio::main]
async fn main() -> Result<()> {
    let path = "composer.json";
    let lock_path = "composer.lock";

    let content = fs::read_to_string(path).context("Read failed")?;
    let manifest: ComposerManifest = serde_json::from_str(&content)?;

    // We will populate this list from either the Lockfile OR the Resolver
    let packages_to_install: Vec<PackageVersion>;

    if Path::new(lock_path).exists() {
        // install from lockfile
        println!("{}", "Lockfile found. Installing from composer.lock...".bold().cyan());
        let lockfile = lock::LockFile::load(lock_path).context("Failed to read lockfile")?;
        packages_to_install = lockfile.packages;
        println!("   Loaded {} packages from lock.", packages_to_install.len());
    } else {
        // resolve dependencies & update
        
        println!("{}", "No lockfile found. Resolving dependencies...".bold().cyan());
        let client = RegistryClient::new();
        let mut queue: VecDeque<(String, String)> = VecDeque::new();
        let mut installed_set: HashSet<String> = HashSet::new();
        let mut resolved_packages: Vec<PackageVersion> = Vec::new();

        let start_time = std::time::Instant::now();

        // Prime queue
        for (name, constraint) in manifest.require {
            queue.push_back((name, constraint));
        }

        while let Some((pkg_name, version_constraint)) = queue.pop_front() {
            if installed_set.contains(&pkg_name) { continue; }

            let best_package = match resolve_package(&client, &pkg_name, &version_constraint).await? {
                Some(pkg) => pkg,
                None => {
                    eprintln!("{} Could not resolve {} {}", "Warning:".yellow().bold(), pkg_name, version_constraint);
                    continue;
                }
            };

            println!("   Locked: {} {}", best_package.name.as_deref().unwrap_or(&pkg_name).green(), best_package.version.green());

            // Add dependencies to queue
            for (dep_name, dep_constraint) in &best_package.require {
                if dep_name == "php" || dep_name.starts_with("ext-") { continue; }
                if !installed_set.contains(dep_name) {
                    queue.push_back((dep_name.clone(), dep_constraint.clone()));
                }
            }

            installed_set.insert(pkg_name.clone());
            resolved_packages.push(best_package);
        }

        println!("{}", format!("Resolution complete in {:.2?}", start_time.elapsed()).bold());

        // WRITE LOCKFILE
        let lock_data = lock::LockFile::new(resolved_packages.clone());
        lock_data.save(lock_path)?;
        println!("{}", "Generated composer.lock".green());

        packages_to_install = resolved_packages;
    }

    // Convert PackageVersion structs into the download format (name, version, url)
    let mut download_list = Vec::new();
    for pkg in &packages_to_install {
        if let Some(dist) = &pkg.dist {
            // Some packages might have missing names in the struct if they came from minimal JSON,
            // but usually they are populated.
            let name = pkg.name.clone().unwrap_or_else(|| "unknown".to_string());
            download_list.push((name, pkg.version.clone(), dist.url.clone()));
        }
    }

    println!("{}", format!("Starting parallel download of {} packages...", download_list.len()).cyan());

    let mut set = JoinSet::new();
    for (name, version, url) in download_list {
        set.spawn(async move {
            installer::install_package(&name, &version, &url).await
        });
    }

    let mut success_count = 0;
    while let Some(res) = set.join_next().await {
        match res {
            Ok(Ok(_)) => success_count += 1,
            Ok(Err(e)) => eprintln!("{} {}", "Download failed:".red().bold(), e),
            Err(e) => eprintln!("{} {}", "Task panic:".red().bold(), e),
        }
    }

    println!("{} All {} packages installed successfully!", "Success:".green().bold(), success_count);

    generator::generate_autoload("vendor")?;
    println!("{} Autoload files generated.", "Success:".green().bold());

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
    Ok(compatible_versions.last().map(|(pkg, _)| (**pkg).clone()))
}