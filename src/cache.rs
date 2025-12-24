use std::fs;
use std::path::{Path, PathBuf};
use anyhow::Result;
use dirs::cache_dir;

pub struct Cache {
    root: PathBuf,
}

impl Cache {
    pub fn new() -> Self {
        // Linux/Mac: ~/.cache/maestro
        // Windows: C:\Users\Name\AppData\Local\Maestro
        let mut path = cache_dir().unwrap_or_else(|| PathBuf::from(".cache"));
        path.push("maestro");

        fs::create_dir_all(path.join("metadata")).ok();
        fs::create_dir_all(path.join("dist")).ok();

        Self { root: path }
    }

    // get path for metadata cache: ~/.cache/maestro/metadata/vendor-package.json
    pub fn get_metadata_path(&self, pkg_name: &str) -> PathBuf {
        let safe_name = pkg_name.replace('/', "-");
        self.root.join("metadata").join(format!("{}.json", safe_name))
    }

    // Get path for dist cache: ~/.cache/metadata/dist/vendor-package-version.zip
    pub fn get_dist_path(&self, pkg_name: &str, version: &str) -> PathBuf {
        let safe_name = pkg_name.replace('/', "-");
        self.root.join("dist").join(format!("{}-{}.zip", safe_name, version))
    }
}