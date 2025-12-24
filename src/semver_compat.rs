use semver::{Version, VersionReq};

pub fn to_rust_version(php_version: &str) -> Option<Version> {
    // packagist normalized versions are typically "1.2.3.0"
    // strip the last ".0" if its there
    let clean_version = if php_version.matches('.').count() == 3 {
        let last_dot = php_version.rfind('.').unwrap();
        &php_version[..last_dot]
    } else {
        php_version
    };

    Version::parse(clean_version).ok()

}

pub fn version_matches(req_str: &str, version_str: &str) -> bool {
    let v = match to_rust_version(version_str) {
        Some(found) => found,
        None => return false
    };

    // PHP uses "||" for OR
    // semver doesn't natively support "||" in a single string
    // split by "||" and check if any part matches
    for part in req_str.split("||") {
        let clean_part = part.trim();

        // valid, check if it matches
        if let Ok(req) = VersionReq::parse(clean_part) {
            if req.matches(&v) {
                return true;
            }
        }
    }

    false
}