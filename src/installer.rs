use std::fs;
use std::io::Cursor;
use std::path::Path;
use anyhow::{Context, Result};
use crate::cache::Cache;

pub async fn install_package(name: &str, version: &str, url: &str) -> Result<()> {
    let cache = Cache::new();
    let cache_path = cache.get_dist_path(name, version);

    let bytes: Vec<u8>;

    if cache_path.exists() {
        println!("    Using cached: {}", name);
        bytes = fs::read(&cache_path)?;
    } else {
        println!("Downloading {}...", name);
        println!("Downloading {} v{}...", name, version);

        let client = reqwest::Client::builder()
            .user_agent("Maestro/0.1")
            .build()?;

        let response = client.get(url).send().await?;
        if !response.status().is_success() {
            anyhow::bail!("Download failed: {}", response.status());
        }
        bytes = response.bytes().await?.to_vec();

        // save to cache
        let _ = fs::write(&cache_path, &bytes);
    }

    // extract
    let install_dir = format!("vendor/{}", name);
    let path = Path::new(&install_dir);
    if path.exists() {
        fs::remove_dir_all(path).context("Failed to clean existing directory")?;
    }
    fs::create_dir_all(path).context("Failed to create vendor directory")?;

    println!("Extracting to {}...", install_dir);
    let cursor = Cursor::new(bytes); // Wrap bytes so zip can read them
    let mut archive = zip::ZipArchive::new(cursor).context("Failed to read zip archive")?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;

        let filepath = match file.enclosed_name() {
            Some(path) => path,
            None => continue,
        };

        // Skip the root folder entry itself
        if file.is_dir() {
            continue;
        }

        // Strip the first directory component
        let mut components = filepath.components();
        components.next(); // Skip root
        let relative_path = components.as_path();

        if relative_path.as_os_str().is_empty() {
            continue;
        }

        let outpath = path.join(relative_path);

        if let Some(p) = outpath.parent() {
            if !p.exists() {
                fs::create_dir_all(p)?;
            }
        }

        let mut outfile = fs::File::create(&outpath)?;
        std::io::copy(&mut file, &mut outfile)?;
    }

    println!("Installed {} v{}", name, version);

    Ok(())
}