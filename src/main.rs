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
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "Maestro")]
#[command(version = "0.1")]
#[command(about = "A blazing fast PHP package manager written in Rust", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Install,
    Update,
    Add {
        name: String
    },
}


#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // default: "install"
    match cli.command.unwrap_or(Commands::Install) {
        Commands::Install => run_install().await?,
        Commands::Update => run_update().await?,
        Commands::Add { name } => run_add(&name).await?,
    }

    Ok(())
}

async fn run_install() -> Result<()> {
    let lock_path = "composer.lock";
    if Path::new(lock_path).exists() {
        println!("{}", "Lockfile found. Installing locked dependencies...".bold().cyan());
        let lockfile = lock::LockFile::load(lock_path).context("Failed to read lockfile")?;
        download_and_install(lockfile.packages).await?;
    } else {
        println!("{}", "No lockfile found. Running resolution...".bold().cyan());
        run_update().await?;
    }

    Ok(())
}

async fn run_update() -> Result<()> {
    println!("{}", "Updating dependencies...".bold().cyan());

    let path = "composer.json";
    let lock_path = "composer.lock";

    let content = fs::read_to_string(path).context("Read composer.json failed")?;
    let manifest: ComposerManifest = serde_json::from_str(&content)?;
    let client = RegistryClient::new();

    let mut queue: VecDeque<(String, String)> = VecDeque::new();
    let mut installed_set: HashSet<String> = HashSet::new();
    let mut resolved_packages: Vec<PackageVersion> = Vec::new();

    let start_time = std::time::Instant::now();

    for (name, constraint) in manifest.require {
        queue.push_back((name, constraint));
    }

    while let Some((pkg_name, version_constraint)) = queue.pop_front() {
        if installed_set.contains(&pkg_name) { continue }

        let best_package = match resolve_package(&client, &pkg_name, &version_constraint).await? {
            Some(pkg) => pkg,
            None => {
                eprintln!("{} Could not resolve {} {}", "Warning:".yellow().bold(), pkg_name, version_constraint);
                continue;
            }
        };

        println!("   Locked: {} {}", best_package.name.as_deref().unwrap_or(&pkg_name).green(), best_package.version.green());

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

    let lock_data = lock::LockFile::new(resolved_packages.clone());
    lock_data.save(lock_path)?;
    println!("{}", "Generated composer.lock".green());
    Ok(())
}

async fn download_and_install(packages: Vec<PackageVersion>) -> Result<()> {
    let mut download_list = Vec::new();
    for pkg in &packages {
        if let Some(dist) = &pkg.dist {
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

async fn run_add(pkg_name: &str) -> Result<()> {
    println!("{}", format!("Adding {}...", pkg_name).bold().cyan());

    let client = RegistryClient::new();
    let versions = client.get_package_metadata(pkg_name).await.context("Could not find package.")?;

    // find latest STABLE version
    let latest = versions.iter()
        .filter(|v| {
            let s = &v.version_normalized;
            !s.contains("dev") && !s.contains("alpha") && !s.contains("beta") && !s.contains("RC")
        })
        .max_by_key(|v| to_rust_version(&v.version_normalized));

    let target_version = match latest {
        Some(v) => format!("^{}", v.version),
        None => {
            println!("{}", "Warning: No stable version found.  Using latest unstable.".yellow());
            // fallback to latest if no stable exists
            let v = versions.last().unwrap();
            v.version.clone()
        }
    };

    println!("    Selected version: {}", target_version.green());

    // edit composer.json
    let path = "composer.json";
    let content = fs::read_to_string(path).context("Read composer.json failed")?;
    let mut manifest: ComposerManifest = serde_json::from_str(&content)?;

    // insert new requirement
    manifest.require.insert(pkg_name.to_string(), target_version);

    let new_content = serde_json::to_string_pretty(&manifest)?;
    fs::write(path, new_content)?;

    println!("{}", format!("Added {} to composer.json", pkg_name).green());

    run_update().await
}