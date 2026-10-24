use std::path::Path;

use ets_go::sprintf;
use ets_shim::core::json;

use crate::*;

// go test: etscore/version_generated_test.go:11 TestEffectVersionMatchesPackageJSON
#[test]
fn test_effect_version_matches_package_json() {
    ets_go::testing::run(|t| {
        // Rust: Go resolves its own file with runtime.Caller; the crate lives at crates/ets_etscore.
        let package_json_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("_packages")
            .join("tsgo")
            .join("package.json")
            .display()
            .to_string();
        let content = match std::fs::read_to_string(&package_json_path) {
            Ok(content) => content,
            Err(err) => {
                t.fatal(sprintf!("failed to read %s: %v", package_json_path, err.to_string()))
            }
        };

        let pkg = match json::unmarshal(&content) {
            Ok(pkg) => pkg,
            Err(err) => t.fatal(sprintf!("failed to parse %s: %v", package_json_path, err)),
        };
        // Rust: Go decodes into `struct { Version string }`, which leaves a missing version empty.
        let version = match &pkg {
            json::Value::Object(fields) => match fields.get("version") {
                Some(json::Value::String(version)) => version.clone(),
                _ => String::new(),
            },
            _ => String::new(),
        };
        if version.is_empty() {
            t.fatal(sprintf!("package.json at %s has empty version", package_json_path));
        }

        if EFFECT_VERSION != version {
            t.fatal(sprintf!(
                "EffectVersion = %q, package.json version = %q",
                EFFECT_VERSION,
                version
            ));
        }
    });
}
