use std::sync::Arc;

use rustc_hash::FxHashSet;

use crate::*;

// go: internal/typeparser/discover.go:8 DiscoveredPackage 5f3bc2803ef1c109
// DiscoveredPackage represents a package found in the program's source files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveredPackage {
    pub(crate) name: String,
    pub(crate) version: Option<String>,
    pub(crate) depends_on_effect: bool,
    pub(crate) package_directory: String,
}

impl DiscoveredPackage {
    // go: internal/typeparser/discover.go:15 DiscoveredPackage.Name b484f8ce366d7987
    pub fn name(&self) -> &str {
        &self.name
    }

    // go: internal/typeparser/discover.go:17 DiscoveredPackage.Version 4c4d1ee28730119f
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    // go: internal/typeparser/discover.go:24 DiscoveredPackage.DependsOnEffect 95f4728783cb280c
    pub fn depends_on_effect(&self) -> bool {
        self.depends_on_effect
    }

    // go: internal/typeparser/discover.go:26 DiscoveredPackage.PackageDirectory e5100f6a06d197d7
    pub fn package_directory(&self) -> &str {
        &self.package_directory
    }
}

// go: internal/typeparser/discover.go:29 packageKey 7255f0686dfc2d5c
// packageKey is used for deduplication of discovered packages.
#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct PackageKey {
    name: String,
    version: String,
    has_ver: bool,
}

// go: internal/typeparser/discover.go:36 EffectMajorVersion 87b9e2c45dd651d2
// go: internal/typeparser/discover.go:38 EffectMajorUnknown 0268237f075c828e
// EffectMajorVersion represents the detected major version of the Effect library.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum EffectMajorVersion {
    #[default]
    Unknown = 0,
    V3 = 3,
    V4 = 4,
}

// go: internal/typeparser/discover.go:45 EffectMajorVersion.String 2fb47a39dd31f7d1
// String returns the string representation of the Effect major version.
//
// Rust: Go's `String` method is `Display` (`to_string()`).
impl std::fmt::Display for EffectMajorVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            EffectMajorVersion::V3 => "v3",
            EffectMajorVersion::V4 => "v4",
            EffectMajorVersion::Unknown => "unknown",
        })
    }
}

impl TypeParser<'_> {
    // go: internal/typeparser/discover.go:59 TypeParser.DetectEffectVersion 18940f33e739d55e
    // DetectEffectVersion detects the major version of the Effect library from the program's
    // source files. Returns EffectMajorUnknown if no Effect dependency is found or if
    // conflicting versions are detected. The result is cached per checker lifetime.
    pub fn detect_effect_version(&mut self) -> EffectMajorVersion {
        {
            let links = self.links();
            if links.detect_effect_version_computed {
                return links.detect_effect_version_value;
            }
        }
        let result = self.detect_effect_version_uncached();
        let mut links = self.links();
        links.detect_effect_version_value = result;
        links.detect_effect_version_computed = true;
        result
    }

    // go: internal/typeparser/discover.go:73 TypeParser.detectEffectVersionUncached 417a9af6270a37bd
    // detectEffectVersionUncached performs the actual version detection logic.
    pub(crate) fn detect_effect_version_uncached(&mut self) -> EffectMajorVersion {
        let packages = self.discover_packages();

        let mut detected = EffectMajorVersion::Unknown;
        let mut found = false;

        for (_, pkg) in ets_readonly::new(packages.as_deref()).all() {
            if pkg.name != "effect" {
                continue;
            }

            let major = match &pkg.version {
                None => EffectMajorVersion::Unknown,
                Some(version) if !version.is_empty() => match version.as_bytes()[0] {
                    b'3' => EffectMajorVersion::V3,
                    b'4' => EffectMajorVersion::V4,
                    _ => EffectMajorVersion::Unknown,
                },
                Some(_) => EffectMajorVersion::Unknown,
            };

            if !found {
                detected = major;
                found = true;
            } else if detected != major {
                return EffectMajorVersion::Unknown;
            }
        }

        if !found {
            return EffectMajorVersion::Unknown;
        }
        detected
    }

    // go: internal/typeparser/discover.go:119 TypeParser.SupportedEffectVersion 9f7af95d9c87592a
    // SupportedEffectVersion returns the normalized supported Effect major version.
    // It returns EffectMajorV4 when v4 is detected, and EffectMajorV3 for all other
    // outcomes (including unknown). This is the central extension point for future
    // compiler-option-based version forcing. The result is cached per checker lifetime.
    pub fn supported_effect_version(&mut self) -> EffectMajorVersion {
        if self.detect_effect_version() == EffectMajorVersion::V4 {
            return EffectMajorVersion::V4;
        }
        EffectMajorVersion::V3
    }

    // go: internal/typeparser/discover.go:132 TypeParser.DetectEffectVersionString 9fbd333b6b27435a
    // DetectEffectVersionString returns the exact version string of the Effect library.
    // Returns "unknown" if no Effect dependency is found, if the version is nil, or if
    // conflicting versions are detected.
    //
    // Rust: Go's nil checks on the parser and its checker drop (a `TypeParser` always has both).
    pub fn detect_effect_version_string(&mut self) -> String {
        let packages = self.discover_packages();

        let mut detected = String::new();
        let mut found = false;

        for (_, pkg) in ets_readonly::new(packages.as_deref()).all() {
            let Some(version) = &pkg.version else {
                continue;
            };
            if pkg.name != "effect" {
                continue;
            }

            if !found {
                detected.clone_from(version);
                found = true;
            } else if detected != *version {
                return "unknown".to_string();
            }
        }

        if !found {
            return "unknown".to_string();
        }
        detected
    }

    // go: internal/typeparser/discover.go:165 TypeParser.DiscoverPackages eb533152fed0885f
    // DiscoverPackages iterates all source files in the program, resolves each one's
    // nearest package.json, and returns a deduplicated list of discovered packages.
    // Results are cached per checker so repeated calls within the same check cycle
    // (from DetectEffectVersion, DetectEffectVersionString, duplicatePackage rule, etc.)
    // do not re-scan all source files.
    //
    // Rust: Go returns a read-only view of the cached slice; here the cache is behind the links' lock, so it hands
    // out a shared handle (`ets_readonly::new(result.as_deref())` is Go's view; `None` is a nil slice).
    pub fn discover_packages(&mut self) -> Option<Arc<[DiscoveredPackage]>> {
        {
            let links = self.links();
            if links.discover_packages_computed {
                return links.discover_packages_value.clone();
            }
        }

        let result = self.discover_packages_uncached().map(Arc::from);
        let mut links = self.links();
        links.discover_packages_value.clone_from(&result);
        links.discover_packages_computed = true;
        result
    }

    // go: internal/typeparser/discover.go:181 TypeParser.discoverPackagesUncached c8a21ed3befccd80
    // discoverPackagesUncached performs the actual source file scan.
    //
    // Rust: Go's assertions to `sourceFileProgram` and `packageJsonProgram` succeed for the compiler program (see
    // `TypeParser`). A nil result (no package) is `None`.
    pub(crate) fn discover_packages_uncached(&mut self) -> Option<Vec<DiscoveredPackage>> {
        let prog: &dyn SourceFileProgram = self.program;

        let pj_prog: &dyn PackageJsonProgram = self.program;

        let mut seen: FxHashSet<PackageKey> = FxHashSet::default();
        let mut result: Option<Vec<DiscoveredPackage>> = None;
        for &sf in prog.source_files() {
            let Some(pkg) = self.package_json_for_source_file(sf) else {
                continue;
            };

            let Some(name) = pkg.name.get_value() else {
                continue;
            };

            let mut key = PackageKey { name: name.clone(), version: String::new(), has_ver: false };
            let mut ver_ptr: Option<String> = None;

            if let Some(ver) = pkg.version.get_value() {
                key.version.clone_from(ver);
                key.has_ver = true;
                ver_ptr = Some(ver.clone());
            }

            if seen.contains(&key) {
                continue;
            }
            seen.insert(key);

            let mut depends_on_effect = false;
            if let Some(peer_deps) = pkg.peer_dependencies.get_value() {
                depends_on_effect = peer_deps.contains_key("effect");
            }

            let meta = pj_prog.get_source_file_meta_data(sf.path());
            let pkg_dir = meta.package_json_directory;

            result.get_or_insert_with(Vec::new).push(DiscoveredPackage {
                name: name.clone(),
                version: ver_ptr,
                depends_on_effect,
                package_directory: pkg_dir,
            });
        }

        result
    }
}
