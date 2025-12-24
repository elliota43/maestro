use semver::{Version, VersionReq};

pub fn to_rust_version(v: &str) -> Option<Version> {
    // convert v1.2.3 -> 1.2.3
   let clean = v.trim_start_matches('v');

    let parts: Vec<&str> = clean.split('.').collect();
    let standardized = if parts.len() > 3 {
        format!("{}.{}.{}", parts[0], parts[1], parts[2])
    } else {
        clean.to_string()
    };

    Version::parse(&standardized).ok()
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