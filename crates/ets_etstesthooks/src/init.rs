use ets_bundledeffect::EffectVersion;
use ets_fourslash::{TestFS, TestFSEntry};

// go test: etstesthooks/init.go:10 init d8940441b111151e
pub(crate) fn init() {
    ets_fourslash::register_prepare_test_fs_callback(prepare_test_fs);
}

// go test: etstesthooks/init.go:16 prepareTestFS d9411af5e0035b2b
// prepareTestFS detects Effect imports or explicit version markers in test files
// and mounts real Effect packages into the fourslash VFS.
pub(crate) fn prepare_test_fs(testfs: &mut TestFS) {
    let mut has_effect_import = false;
    let mut has_vitest_import = false;
    let mut has_v3_marker = false;
    let mut has_v4_marker = false;
    #[expect(
        clippy::iter_over_hash_type,
        reason = "Go ranges over the map too; only flags are set, so the order is not observable"
    )]
    for v in testfs.values() {
        let TestFSEntry::String(content) = v else {
            continue;
        };
        if content.contains(r#"from "effect"#) {
            has_effect_import = true;
        }
        if content.contains("vitest") {
            has_vitest_import = true;
        }
        if content.starts_with("// @effect-v3") || content.contains("\n// @effect-v3") {
            has_v3_marker = true;
        }
        if content.starts_with("// @effect-v4") || content.contains("\n// @effect-v4") {
            has_v4_marker = true;
        }
    }
    if !has_effect_import && !has_vitest_import && !has_v3_marker && !has_v4_marker {
        return;
    }
    let mut version = EffectVersion::V4;
    if has_v3_marker {
        version = EffectVersion::V3;
    }
    if let Err(err) = mount(version, testfs, ets_bundledeffect::mount_effect) {
        panic!("{err}");
    }
    if has_vitest_import {
        if let Err(err) = mount(version, testfs, ets_bundledeffect::mount_vitest) {
            panic!("{err}");
        }
    }
}

/// Rust-only: Go's `bundledeffect.MountEffect(version, testfs)` / `MountVitest` write into the fourslash
/// `map[string]any`; the Rust functions fill an `ets_bundledeffect::TestFS`, whose files are copied in as the
/// `*fstest.MapFile` entries Go adds.
fn mount(
    version: EffectVersion,
    testfs: &mut TestFS,
    mount: fn(EffectVersion, &mut ets_bundledeffect::TestFS) -> Result<(), ets_go::Error>,
) -> Result<(), ets_go::Error> {
    let mut mounted = ets_bundledeffect::TestFS::default();
    mount(version, &mut mounted)?;
    #[expect(
        clippy::iter_over_hash_type,
        reason = "inserting into a map: the order is not observable"
    )]
    for (path, file) in mounted {
        testfs.insert(path, TestFSEntry::MapFile(file));
    }
    Ok(())
}
