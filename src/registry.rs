use std::collections::HashMap;
use serde::Deserialize;
use anyhow::{Result, Context};

#[derive(Debug, Deserialize)]
pub struct PackagistResponse {
    pub packages: HashMap<String, Vec<PackageVersion>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PackageVersion {
    pub name: String,
    pub version: String,
    pub version_normalized: String,
    pub require: HashMap<String, String>,
    pub dist: Option<DistInfo>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DistInfo {
    pub url: String,
    pub r#type: String,
    pub reference: Option<String>, // commit hash
    pub shasum: Option<String>,
}

pub struct RegistryClient {
    client: reqwest::Client,
    base_url: String,
}

impl RegistryClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: "https://repo.packagist.org/p2".to_string(),
        }
    }

    pub async fn get_package_metadata(&self, name: &str) -> Result<Vec<PackageVersion>> {
        let url = format!("{}/{}.json", self.base_url, name);
        println!("Fetching metadata for: {}", url);

        let resp = self.client.get(&url)
            .send()
            .await
            .context("Failed to connect to Packagist")?
            .json::<PackagistResponse>()
            .await
            .context("Failed to parse Packagist JSON response")?;


        resp.packages
            .get(name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Package {} not found in response", name))
    }
}