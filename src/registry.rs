use std::collections::HashMap;
use serde::{Deserialize, Deserializer, Serialize};
use anyhow::{Result, Context};
use std::fs;
use crate::cache::Cache;

#[derive(Debug, Deserialize)]
pub struct PackagistResponse {
    pub packages: HashMap<String, Vec<PackageVersion>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PackageVersion {
    pub name: Option<String>,
    pub version: String,

    #[serde(alias = "version_normalized", default)]
    pub version_normalized: String,

    #[serde(default, deserialize_with = "deserialize_packagist_map")]
    pub require: HashMap<String, String>,
    pub dist: Option<DistInfo>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DistInfo {
    pub url: String,
    pub r#type: String,

    #[serde(default)]
    pub reference: Option<String>, // commit hash

    #[serde(default)]
    pub shasum: Option<String>,
}

// Helper fn:
// Packagist sometimes sends "__unset" (str) instead of {}
// this handles those instances so the program doesn't crash
fn deserialize_packagist_map<'de, D>(deserializer: D) -> Result<HashMap<String, String>, D::Error>
where
    D: Deserializer<'de>, {

    // convert to generic JSON Value first
    let v: serde_json::Value = Deserialize::deserialize(deserializer)?;

    match v {
        // std case -> map
        serde_json::Value::Object(obj) => {
            let mut map = HashMap::new();
            for (k, val) in obj {
                if let serde_json::Value::String(s) = val {
                    map.insert(k, s);
                }
            }
            Ok(map)
        }

        // edge case: packagist says "remove all requirements"
        serde_json::Value::String(s) if s == "__unset" => Ok(HashMap::new()),

        // edge case: empty array [] is sometimes sent for empty maps in php
        serde_json::Value::Array(_) => Ok(HashMap::new()),

        // fallback: null or anything else -> empty map
        _ => Ok(HashMap::new()),
    }

}
pub struct RegistryClient {
    client: reqwest::Client,
    base_url: String,
    cache: Cache,
}

impl RegistryClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent("Maestro/0.1")
                .build()
                .unwrap(),
            base_url: "https://repo.packagist.org/p2".to_string(),
            cache: Cache::new(),
        }
    }

    pub async fn get_package_metadata(&self, name: &str) -> Result<Vec<PackageVersion>> {

        // check cache
        let cache_path = self.cache.get_metadata_path(name);

        if cache_path.exists() {
            if let Ok(content) = fs::read_to_string(&cache_path) {
                if let Ok(parsed) = serde_json::from_str::<PackagistResponse>(&content) {
                    if let Some(versions) = parsed.packages.get(name) {
                        return Ok(versions.clone());
                    }
                }
            }
        }

        let url = format!("{}/{}.json", self.base_url, name);

        let resp = self.client.get(&url)
            .send()
            .await
            .context("Failed to connect to Packagist")?;

        if !resp.status().is_success() {
            anyhow::bail!("Packagist returned error: {}", resp.status());
        }

        let text = resp.text().await?;

        // Write to cache
        if let Err(e) = fs::write(&cache_path, &text) {
            eprintln!("Warning: Failed to write cache: {}", e);
        }

        let parsed: PackagistResponse = serde_json::from_str(&text)?;

        let mut versions = parsed.packages
            .get(name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Package {} not found", name))?;

        for v in &mut versions {
            if v.name.is_none() {
                v.name = Some(name.to_string());
            }
        }
        Ok(versions)
    }
}