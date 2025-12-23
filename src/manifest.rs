use std::collections::HashMap;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ComposerManifest {
    pub name: Option<String>,
    pub description: Option<String>,

    // dependencies
    #[serde(default)]
    pub require: HashMap<String, String>,

    #[serde(default, rename = "require-dev")]
    pub require_dev: HashMap<String, String>,

    // capture other fields as a generic value to not lose data
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}