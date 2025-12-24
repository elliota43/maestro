mod manifest;
mod registry;
mod semver_compat;
mod installer;
mod generator;
mod cache;

use manifest::ComposerManifest;
use registry::{RegistryClient, PackageVersion};
use semver_compat::to_rust_version;
use anyhow::{Context, Result};
use std::collections::{VecDeque, HashSet};
use std::fs;
use tokio::task::JoinSet;
use colored::Colorize;

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

    // Store install tasks
    let mut download_list: Vec<(String, String, String)> = Vec::new();

    // prime queue with root dependencies
    for (name, constraint) in manifest.require {
        queue.push_back((name, constraint));
    }

    println!("{}", "Resolving dependency graph...".bold().cyan());
    let start_time = std::time::Instant::now();
    // process queue
    while let Some((pkg_name, version_constraint)) = queue.pop_front() {
        if installed.contains(&pkg_name) {
            continue;
        }

        let best_package = match resolve_package(&client, &pkg_name, &version_constraint).await? {
            Some(pkg) => pkg,
            None => {
                eprintln!("{} Warning: Could not resolve {} {}", "Warning:".yellow().bold(), pkg_name, version_constraint);
                continue;
            }
        };

        println!(
            "    - Locked: {} v{}",
            best_package.name.as_deref().unwrap_or(&pkg_name).green(),
            best_package.version.green()
        );

        // Queue for Download
        if let Some(dist) = best_package.dist {
            download_list.push((
                pkg_name.clone(),
                best_package.version.clone(),
                dist.url.clone()

            ));
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

    println!("{}", format!("Resolution complete in {:.2?}", start_time.elapsed()).bold());    println!("Starting parallel download of {} packages...", download_list.len());
    println!("{}", format!("Starting parallel download of {} packages...", download_list.len()).cyan());

    // install (parallel)
    let mut set = JoinSet::new();

    for (name, version, url) in download_list {
        // spawn lightweight thread for each download
        set.spawn(async move {
            installer::install_package(&name, &version, &url).await
        });
    }

    let mut success_count = 0;
    while let Some(res) = set.join_next().await {
        match res {
            Ok(Ok(_)) => success_count += 1,
            Ok(Err(e)) => eprintln!("{} {} ", "Download failed:".red().bold(), e),
            Err(e) => eprintln!("{} {}", "Task panic:".red().bold(), e),
        }
    }

    println!("{} All {} packages installed successfully!", "Success:".green().bold(), success_count);

    // generate autoload
    generator::generate_autoload("vendor")?;
    println!("{} Autoload files generated:", "Success:".green().bold());
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