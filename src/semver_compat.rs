use semver::{Version, VersionReq};
use anyhow::Result;

pub fn to_rust_version(php_version: &str) -> Option<Version> {
    let clean_version = if php_version.matches('.').count() == 3 {
        let last_dot = php_version.rfind('.').unwrap();
        &php_version[..last_dot]
    } else {
        php_version
    };

    Version::parse(clean_version).ok()

}

pub fn version_matches(req_str: &str, version_str: &str) -> bool {
    let req = match VersionReq::parse(req_str) {
        Ok(r) => r,
        Err(_) => return false,
    };

    if let Some(v) = to_rust_version(version_str) {
        req.matches(&v)
    } else {
        false
    }
}