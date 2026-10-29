use std::sync::{Arc, LazyLock, OnceLock};
use std::time::SystemTime;

use ets_bundledeffect::{EffectVersion, TestFS};
use ets_go::Error;
use ets_shim::vfs::{Entries, FS, FileInfo, vfstest};
use parking_lot::Mutex;
use rustc_hash::{FxHashMap, FxHashSet};

// go test: internal/effecttest/sharedfs.go:18 sharedPackages 6621c7d8c6ceabcd
// sharedPackages is a read-only VFS of the packages MountEffect (and
// MountVitest) mount, built once per version and shared by every test, so a
// test does not copy, sort and re-index thousands of package files.
pub(crate) struct SharedPackages {
    fs: Box<dyn FS>,
    // contents holds the file contents as strings, so reads do not copy them.
    //
    // Rust: `FS::read_file` returns an owned `String`, so a read still clones the content.
    contents: FxHashMap<String, String>,
    pub(crate) package_json: Vec<u8>,
}

// go test: internal/effecttest/sharedfs.go:25 sharedPackagesKey 5e0ad5d07e9d6367
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct SharedPackagesKey {
    version: EffectVersion,
    vitest: bool,
}

type SharedPackagesResult = Result<Arc<SharedPackages>, Error>;

// go test: internal/effecttest/sharedfs.go:30 sharedPackagesCache 8d3b3f87a75e510d
// Rust: Go's `sync.Map` of `sync.OnceValues` loaders; a loader is a shared `OnceLock`.
static SHARED_PACKAGES_CACHE: LazyLock<
    Mutex<FxHashMap<SharedPackagesKey, Arc<OnceLock<SharedPackagesResult>>>>,
> = LazyLock::new(Mutex::default);

// go test: internal/effecttest/sharedfs.go:32 getSharedPackages d33dbf34e3c11243
pub(crate) fn get_shared_packages(version: EffectVersion, vitest: bool) -> SharedPackagesResult {
    let key = SharedPackagesKey { version, vitest };
    let load = {
        let mut cache = SHARED_PACKAGES_CACHE.lock();
        Arc::clone(cache.entry(key).or_default())
    };
    load.get_or_init(|| {
        let mut files = TestFS::default();
        ets_bundledeffect::mount_effect(version, &mut files)?;
        if vitest {
            ets_bundledeffect::mount_vitest(version, &mut files)?;
        }
        // The project's package.json lives in the test's own directory.
        let package_json =
            files.remove("/.src/package.json").expect("MountEffect mounts /.src/package.json").data;
        let contents = files
            .iter()
            .map(|(name, file)| (name.clone(), String::from_utf8_lossy(&file.data).into_owned()))
            .collect();
        Ok(Arc::new(SharedPackages {
            fs: Box::new(vfstest::from_map(files, true)),
            contents,
            package_json,
        }))
    })
    .clone()
}

// go test: internal/effecttest/sharedfs.go:63 overlayFS ab70ae2df70b6a07
// overlayFS layers a test's own files over the shared packages. The test's
// files win; directory listings merge both layers, since tests add their own
// packages (and files) under /node_modules. Writes go to the test's layer.
pub(crate) struct OverlayFS<U: FS> {
    upper: U,
    shared: Arc<SharedPackages>,
}

// go test: internal/effecttest/sharedfs.go:68 newOverlayFS 7b2a31a9fca7801b
pub(crate) fn new_overlay_fs<U: FS>(upper: U, shared: Arc<SharedPackages>) -> OverlayFS<U> {
    OverlayFS { upper, shared }
}

impl<U: FS> FS for OverlayFS<U> {
    // go test: internal/effecttest/sharedfs.go:72 overlayFS.CaseSensitivity 69398a1557c61b4b
    // Rust: tsrs names it `use_case_sensitive_file_names`.
    fn use_case_sensitive_file_names(&self) -> bool {
        self.upper.use_case_sensitive_file_names()
    }

    // go test: internal/effecttest/sharedfs.go:74 overlayFS.FileExists 7a095b0684930e2c
    fn file_exists(&self, path: &str) -> bool {
        if self.upper.file_exists(path) {
            return true;
        }
        self.shared.contents.contains_key(path)
    }

    // go test: internal/effecttest/sharedfs.go:82 overlayFS.ReadFile 85e571ddb44569f2
    fn read_file(&self, path: &str) -> Option<String> {
        if let Some(contents) = self.upper.read_file(path) {
            return Some(contents);
        }
        self.shared.contents.get(path).cloned()
    }

    // go test: internal/effecttest/sharedfs.go:90 overlayFS.WriteFile e43701c1aa6e32ea
    fn write_file(&self, path: &str, data: &str) -> Result<(), String> {
        self.upper.write_file(path, data)
    }

    // go test: internal/effecttest/sharedfs.go:94 overlayFS.AppendFile 602ffce1a7a8516a
    fn append_file(&self, path: &str, data: &str) -> Result<(), String> {
        self.upper.append_file(path, data)
    }

    // go test: internal/effecttest/sharedfs.go:98 overlayFS.Remove 8c6d5339aa9eb920
    fn remove(&self, path: &str) -> Result<(), String> {
        self.upper.remove(path)
    }

    // go test: internal/effecttest/sharedfs.go:100 overlayFS.Chtimes 19620c061860aa55
    fn chtimes(&self, path: &str, a_time: SystemTime, m_time: SystemTime) -> Result<(), String> {
        self.upper.chtimes(path, a_time, m_time)
    }

    // go test: internal/effecttest/sharedfs.go:104 overlayFS.DirectoryExists b92c8cdd5be8a30f
    fn directory_exists(&self, path: &str) -> bool {
        self.upper.directory_exists(path) || self.shared.fs.directory_exists(path)
    }

    // go test: internal/effecttest/sharedfs.go:108 overlayFS.GetAccessibleEntries 09f2d5caa940ed8d
    fn get_accessible_entries(&self, path: &str) -> Entries {
        if !self.shared.fs.directory_exists(path) {
            return self.upper.get_accessible_entries(path);
        }
        let shared = self.shared.fs.get_accessible_entries(path);
        if !self.upper.directory_exists(path) {
            return shared;
        }
        let upper = self.upper.get_accessible_entries(path);
        let symlinks = if upper.symlinks.is_some() || shared.symlinks.is_some() {
            let mut merged = FxHashSet::default();
            merged.extend(upper.symlinks.into_iter().flatten());
            merged.extend(shared.symlinks.into_iter().flatten());
            Some(merged)
        } else {
            None
        };
        Entries {
            files: merge_names(upper.files, shared.files),
            directories: merge_names(upper.directories, shared.directories),
            symlinks,
        }
    }

    // go test: internal/effecttest/sharedfs.go:139 overlayFS.Stat 49ad60ac251c9365
    fn stat(&self, path: &str) -> Option<FileInfo> {
        if let Some(info) = self.upper.stat(path) {
            return Some(info);
        }
        self.shared.fs.stat(path)
    }

    // go test: internal/effecttest/sharedfs.go:146 overlayFS.Realpath 303abab4e9d5a3aa
    fn realpath(&self, path: &str) -> String {
        if self.upper.stat(path).is_some() {
            return self.upper.realpath(path);
        }
        self.shared.fs.realpath(path)
    }
}

// go test: internal/effecttest/sharedfs.go:133 mergeNames bd195af016ccc50a
fn merge_names(a: Vec<String>, b: Vec<String>) -> Vec<String> {
    let mut merged = [a, b].concat();
    merged.sort();
    merged.dedup();
    merged
}
