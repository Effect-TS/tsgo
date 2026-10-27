use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, OnceLock};

use ets_go::Error;
use ets_shim::vfs::vfstest::MapFile;
use parking_lot::Mutex;
use rustc_hash::FxHashMap;
use sha2::{Digest, Sha256};

/// Rust-only: Go's `map[string]any` test file system (`fstest.MapFile` values), as `vfstest::from_map` takes it.
pub type TestFS = FxHashMap<String, MapFile>;

// go test: internal/bundledeffect/effect.go:24 EffectVersion 917c453081ce01fe
// go test: internal/bundledeffect/effect.go:26 EffectV3 5d3b4b354020667e
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum EffectVersion {
    V3,
    V4,
}

// go test: internal/bundledeffect/effect.go:26 EffectV3 5d3b4b354020667e
impl EffectVersion {
    /// Rust-only: the Go string value (`string(version)`).
    pub fn as_str(self) -> &'static str {
        match self {
            EffectVersion::V3 => "effect-v3",
            EffectVersion::V4 => "effect-v4",
        }
    }
}

// go test: internal/bundledeffect/effect.go:31 EffectTsGoRootPath 42a6a06ec0c8e2c9
// Rust: Go derives the root from this source file's path (`runtime.Caller`); the crate's manifest directory is
// crates/ets_bundledeffect, two levels below the root.
pub fn effect_ts_go_root_path() -> PathBuf {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    crate_dir.parent().and_then(std::path::Path::parent).expect("crate below crates/").to_path_buf()
}

// go test: internal/bundledeffect/effect.go:39 PackagePath c8df42276492e440
pub fn package_path(version: EffectVersion, package_name: &str) -> PathBuf {
    let mut path = effect_ts_go_root_path()
        .join("testdata")
        .join("tests")
        .join(version.as_str())
        .join("node_modules");
    for part in package_name.split('/') {
        path.push(part);
    }
    path
}

// go test: internal/bundledeffect/effect.go:43 EnsurePackageInstalled 75a134dfb9f207c4
pub fn ensure_package_installed(version: EffectVersion, package_name: &str) -> Result<(), Error> {
    if package_fs_cache(version, package_name).is_empty() {
        return Err(ets_go::errors::new(format!(
            "package not installed at {}",
            package_path(version, package_name).display()
        )));
    }
    Ok(())
}

// go test: internal/bundledeffect/effect.go:50 fixtureProfileManifest 0f70a464e6abd8f4
#[derive(Clone, Default)]
#[cfg_attr(not(test), expect(dead_code, reason = "read by the ported test only, as in Go"))]
pub(crate) struct FixtureProfileManifest {
    pub(crate) requested: FxHashMap<String, String>,
    pub(crate) resolved: FxHashMap<String, String>,
}

// go test: internal/bundledeffect/effect.go:55 fixtureManifest 45745da172895665
#[derive(Clone, Default)]
pub(crate) struct FixtureManifest {
    pub(crate) schema_version: i64,
    #[cfg_attr(not(test), expect(dead_code, reason = "read by the ported test only, as in Go"))]
    pub(crate) profiles: FxHashMap<String, FixtureProfileManifest>,
    pub(crate) tree_sha256: String,
}

// go test: internal/bundledeffect/effect.go:55 fixtureManifest 45745da172895665
// Rust: Go decodes with `encoding/json` into the tagged struct (unknown fields ignored, missing ones zero).
impl FixtureManifest {
    fn decode(content: &[u8]) -> Result<FixtureManifest, serde_json::Error> {
        let value: serde_json::Value = serde_json::from_slice(content)?;
        let string_map = |v: Option<&serde_json::Value>| -> FxHashMap<String, String> {
            v.and_then(serde_json::Value::as_object)
                .map(|o| {
                    o.iter()
                        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string()))
                        .collect()
                })
                .unwrap_or_default()
        };
        let profiles = value
            .get("profiles")
            .and_then(serde_json::Value::as_object)
            .map(|o| {
                o.iter()
                    .map(|(name, profile)| {
                        (
                            name.clone(),
                            FixtureProfileManifest {
                                requested: string_map(profile.get("requested")),
                                resolved: string_map(profile.get("resolved")),
                            },
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(FixtureManifest {
            schema_version: value
                .get("schemaVersion")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or_default(),
            profiles,
            tree_sha256: value
                .get("treeSha256")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string(),
        })
    }
}

// go test: internal/bundledeffect/effect.go:61 fixtureBundle a285a68b69b475af
pub(crate) struct FixtureBundle {
    pub(crate) files: FxHashMap<String, Vec<u8>>,
    #[cfg_attr(not(test), expect(dead_code, reason = "read by the ported test only, as in Go"))]
    pub(crate) manifest: FixtureManifest,
}

// go test: internal/bundledeffect/effect.go:67 fixtureArchive 6877625bdc7f9cdc
// Rust: Go embeds the archive (`//go:embed`), generated by `pnpm setup-repo` (repoctl fixtures); so does this.
static FIXTURE_ARCHIVE: &[u8] =
    include_bytes!("../../../internal/bundledeffect/testfixtures.tar.gz");

// go test: internal/bundledeffect/effect.go:69 fixtures 316026a85fdf88ce
pub(crate) fn fixtures() -> &'static FixtureBundle {
    static FIXTURES: LazyLock<FixtureBundle> = LazyLock::new(|| {
        let gzip_reader = flate2::read::GzDecoder::new(FIXTURE_ARCHIVE);

        let mut files: FxHashMap<String, Vec<u8>> = FxHashMap::default();
        let mut manifest = FixtureManifest::default();
        let mut tar_reader = tar::Archive::new(gzip_reader);
        let entries = tar_reader
            .entries()
            .unwrap_or_else(|err| panic!("Failed to open embedded fixtures: {err}"));
        for entry in entries {
            let mut entry =
                entry.unwrap_or_else(|err| panic!("Failed to read embedded fixtures: {err}"));
            if !entry.header().entry_type().is_file() {
                continue;
            }
            let header_name = String::from_utf8_lossy(&entry.path_bytes()).into_owned();
            let mut content = Vec::new();
            if let Err(err) = entry.read_to_end(&mut content) {
                panic!("Failed to read embedded fixture {header_name}: {err}");
            }
            let name = ets_go::path::clean(&header_name);
            if name == "manifest.json" {
                match FixtureManifest::decode(&content) {
                    Ok(m) => manifest = m,
                    Err(err) => panic!("Failed to decode embedded fixture manifest: {err}"),
                }
                continue;
            }
            assert!(!files.contains_key(&name), "Duplicate embedded fixture: {name}");
            files.insert(name, content);
        }
        assert!(
            manifest.schema_version == 1,
            "Unsupported embedded fixture schema version: {}",
            manifest.schema_version
        );
        let digest = fixture_tree_sha256(&files);
        assert!(
            digest == manifest.tree_sha256,
            "Embedded fixture digest mismatch: expected {}, got {}",
            manifest.tree_sha256,
            digest
        );
        FixtureBundle { files, manifest }
    });
    &FIXTURES
}

// go test: internal/bundledeffect/effect.go:115 fixtureTreeSHA256 4ab7acd2e00878bd
pub(crate) fn fixture_tree_sha256(files: &FxHashMap<String, Vec<u8>>) -> String {
    let mut paths: Vec<&String> = files.keys().collect();
    paths.sort();
    let mut hash = Sha256::new();
    for path in paths {
        hash.update(path.as_bytes());
        hash.update([0u8]);
        let content_hash = Sha256::digest(&files[path]);
        hash.update(content_hash);
    }
    hash.finalize().iter().fold(String::with_capacity(64), |mut s, b| {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
        s
    })
}

// go test: internal/bundledeffect/effect.go:127 cacheKey 2c9e0809c68a5ee9
#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct CacheKey {
    pub(crate) version: EffectVersion,
    pub(crate) package_name: String,
}

// go test: internal/bundledeffect/effect.go:132 fsCacheMu 2c05edd7847e358e
// Rust: Go's map of `sync.OnceValue` loaders behind a mutex; a loader is a shared `OnceLock`.
static FS_CACHES: LazyLock<Mutex<FxHashMap<CacheKey, Arc<OnceLock<Arc<TestFS>>>>>> =
    LazyLock::new(Mutex::default);

// go test: internal/bundledeffect/effect.go:137 packageFSCache 88347674e5efa270
// Rust: Go returns the loader and its callers call it; this returns what the loader returns.
pub(crate) fn package_fs_cache(version: EffectVersion, package_name: &str) -> Arc<TestFS> {
    let key = CacheKey { version, package_name: package_name.to_string() };
    let loader = {
        let mut caches = FS_CACHES.lock();
        Arc::clone(caches.entry(key).or_default())
    };
    Arc::clone(loader.get_or_init(|| {
        let prefix = ets_go::path::join(&[version.as_str(), "node_modules", package_name]) + "/";
        let mut testfs = TestFS::default();
        #[expect(
            clippy::iter_over_hash_type,
            reason = "Go ranges over a map too; the result is a map, so the order is not observable"
        )]
        for (path, content) in &fixtures().files {
            let Some(relative_path) = path.strip_prefix(&prefix) else {
                continue;
            };
            let vfs_path = ets_go::path::join(&["/node_modules", package_name, relative_path]);
            testfs.insert(vfs_path, MapFile::from(content.as_slice()));
        }
        Arc::new(testfs)
    }))
}

// go test: internal/bundledeffect/effect.go:161 MountEffect 7413aafa66c9f4c9
pub fn mount_effect(version: EffectVersion, testfs: &mut TestFS) -> Result<(), Error> {
    let packages = ["effect", "pure-rand", "@standard-schema/spec", "fast-check", "@types/node"];
    for package_name in packages {
        ensure_package_installed(version, package_name)?;
        copy_into(testfs, &package_fs_cache(version, package_name));
    }

    let package_json_path = ets_go::path::join(&[version.as_str(), "package.json"]);
    let Some(package_json) = fixtures().files.get(&package_json_path) else {
        return Err(ets_go::errors::new(format!(
            "package.json not installed at {package_json_path}"
        )));
    };
    testfs.insert("/.src/package.json".to_string(), MapFile::from(package_json.as_slice()));
    Ok(())
}

// go test: internal/bundledeffect/effect.go:179 MountVitest 212aac97251ccfa0
pub fn mount_vitest(version: EffectVersion, testfs: &mut TestFS) -> Result<(), Error> {
    let packages = ["vitest", "@vitest/runner", "@effect/vitest"];
    for package_name in packages {
        ensure_package_installed(version, package_name)?;
        copy_into(testfs, &package_fs_cache(version, package_name));
    }
    Ok(())
}

/// Rust-only: Go's `maps.Copy(dst, src)`.
fn copy_into(dst: &mut TestFS, src: &TestFS) {
    #[expect(
        clippy::iter_over_hash_type,
        reason = "inserting into a map: the order is not observable"
    )]
    for (path, file) in src {
        dst.insert(path.clone(), file.clone());
    }
}
