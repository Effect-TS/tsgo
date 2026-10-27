use std::path::Path;

use ets_go::testing::T;
use rustc_hash::FxHashMap;

use crate::*;

// go test: internal/bundledeffect/effect_test.go:11 packageJSON 8f6a951bcc17620f
pub(crate) struct PackageJson {
    pub(crate) dependencies: FxHashMap<String, String>,
    pub(crate) version: String,
}

// go test: internal/bundledeffect/effect_test.go:16 TestEmbeddedFixturesMatchInstalledPackages bffe35236ac35de6
#[test]
fn test_embedded_fixtures_match_installed_packages() {
    ets_go::testing::run(|t| {
        t.parallel();
        for version in [EffectVersion::V3, EffectVersion::V4] {
            let Some(profile) = fixtures().manifest.profiles.get(version.as_str()) else {
                t.fatal(format!("embedded fixtures do not contain {}", version.as_str()));
            };

            let root = effect_ts_go_root_path();
            let requested = read_package_json(
                t,
                &root.join("testdata").join("tests").join(version.as_str()).join("package.json"),
            );
            if profile.requested != requested.dependencies {
                t.fatal(format!(
                    "embedded requested dependencies for {} do not match package.json",
                    version.as_str()
                ));
            }
            // Go: maps.EqualFunc with an always-true comparison, i.e. the same key sets.
            if profile.requested.len() != profile.resolved.len()
                || profile.requested.keys().any(|k| !profile.resolved.contains_key(k))
            {
                t.fatal(format!(
                    "embedded requested and resolved package sets for {} do not match",
                    version.as_str()
                ));
            }
            #[expect(
                clippy::iter_over_hash_type,
                reason = "Go ranges over the map too; it stops at the first failure"
            )]
            for (package_name, embedded_version) in &profile.resolved {
                let installed =
                    read_package_json(t, &package_path(version, package_name).join("package.json"));
                if &installed.version != embedded_version {
                    t.fatal(format!(
                        "embedded {} version for {} is {}, installed {}",
                        version.as_str(),
                        package_name,
                        embedded_version,
                        installed.version
                    ));
                }
            }
        }
    });
}

// go test: internal/bundledeffect/effect_test.go:41 readPackageJSON f0ad8b0be2909bb6
pub(crate) fn read_package_json(t: &T, path: &Path) -> PackageJson {
    t.helper();
    let content = match std::fs::read(path) {
        Ok(content) => content,
        Err(err) => {
            t.fatal(format!("read {}: {}", path.display(), err));
        }
    };
    let parsed: serde_json::Value = match serde_json::from_slice(&content) {
        Ok(parsed) => parsed,
        Err(err) => {
            t.fatal(format!("decode {}: {}", path.display(), err));
        }
    };
    PackageJson {
        dependencies: parsed
            .get("dependencies")
            .and_then(serde_json::Value::as_object)
            .map(|o| {
                o.iter()
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string()))
                    .collect()
            })
            .unwrap_or_default(),
        version: parsed
            .get("version")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
    }
}
