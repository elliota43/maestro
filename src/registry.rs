use std::collections::HashMap;
use serde::Deserialize;
use anyhow::{Result, Context};

#[derive(Debug, Deserialize)]
pub struct PackagistResponse {
    pub packages: HashMap<String, Vec<PackageVersion>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PackageVersion {
    pub name: Option<String>,
    pub version: String,

    #[serde(alias = "version_normalized", default)]
    pub version_normalized: String,

    #[serde(default)]
    pub require: HashMap<String, String>,
    pub dist: Option<DistInfo>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DistInfo {
    pub url: String,
    pub r#type: String,

    #[serde(default)]
    pub reference: Option<String>, // commit hash

    #[serde(default)]
    pub shasum: Option<String>,
}

pub struct RegistryClient {
    client: reqwest::Client,
    base_url: String,
}

impl RegistryClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent("Maestro/0.1")
                .build()
                .unwrap(),
            base_url: "https://repo.packagist.org/p2".to_string(),
        }
    }

    pub async fn get_package_metadata(&self, name: &str) -> Result<Vec<PackageVersion>> {
        let url = format!("{}/{}.json", self.base_url, name);
        println!("Fetching metadata for: {}", url);

        let resp = self.client.get(&url)
            .send()
            .await
            .context("Failed to connect to Packagist")?;

        if !resp.status().is_success() {
            anyhow::bail!("Packagist returned error: {}", resp.status());
        }

        let text = resp.text().await?;

        let parsed: PackagistResponse = match serde_json::from_str(&text) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("JSON Parse Error: {}", e);
                eprintln!("Response: {:.500}", text);
                anyhow::bail!("Failed to parse Packagist JSON");
            }
        };

        parsed.packages
            .get(name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Package {} not found in response", name))
    }
}