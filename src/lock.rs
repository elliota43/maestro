use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::fs;
use anyhow::Result;
use crate::registry::PackageVersion;

#[derive(Debug, Serialize, Deserialize)]
pub struct LockFile {
    pub _readme: Vec<String>,
    pub content_hash: String, // @todo
    pub packages: Vec<PackageVersion>,
    #[serde(default)]
    pub packages_dev: Vec<PackageVersion>, // Placeholder
}

impl LockFile {
    pub fn new(packages: Vec<PackageVersion>) -> Self {
        Self {
            _readme: vec!["This file locks the dependencies of your project to a known state".into()],
            content_hash: "TODO-hash-of-composer-json".into(),
            packages,
            packages_dev: vec![],
        }
    }

    pub fn load(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let lock: Self = serde_json::from_str(&content)?;
        Ok(lock)
    }

    pub fn save(&self, path: &str) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }
}