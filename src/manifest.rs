use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct ComposerManifest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,

    // dependencies
    #[serde(default)]
    pub require: HashMap<String, String>,

    #[serde(default, rename = "require-dev")]
    pub require_dev: HashMap<String, String>,

    #[serde(default)]
    pub autoload: AutoloadConfig,

    // capture other fields as a generic value to not lose data
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct AutoloadConfig {
    #[serde(default, rename = "psr-4")]
    pub psr4: HashMap<String, String>, // "Monolog\\" => "src/"
    // @todo: add psr-0 classmap
}