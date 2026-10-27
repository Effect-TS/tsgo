use ets_fourslash::{TestFS, TestFSEntry};

use crate::init::*;

/// Rust-only: Go's `map[string]any{path: content}` literal.
fn testfs_of(path: &str, content: &str) -> TestFS {
    let mut testfs = TestFS::default();
    testfs.insert(path.to_string(), TestFSEntry::String(content.to_string()));
    testfs
}

// go test: etstesthooks/init_test.go:9 TestPrepareTestFSMountsEffectForV4MarkerWithoutImport 93c7e0466d2371ce
#[test]
fn test_prepare_test_fs_mounts_effect_for_v4_marker_without_import() {
    ets_go::testing::run(|t| {
        t.parallel();

        let mut testfs = testfs_of("/main.ts", "// @effect-v4\nconst value = 1\n");

        prepare_test_fs(&mut testfs);

        assert_mounted_effect(t, &testfs);
        assert_source_package_name(t, &testfs, "effect-v4-tests");
    });
}

// go test: etstesthooks/init_test.go:22 TestPrepareTestFSMountsEffectForV3MarkerWithoutImport 8081ffcc823cb463
#[test]
fn test_prepare_test_fs_mounts_effect_for_v3_marker_without_import() {
    ets_go::testing::run(|t| {
        t.parallel();

        let mut testfs = testfs_of("/main.ts", "// @effect-v3\nconst value = 1\n");

        prepare_test_fs(&mut testfs);

        assert_mounted_effect(t, &testfs);
        assert_source_package_name(t, &testfs, "effect-v3-tests");
    });
}

// go test: etstesthooks/init_test.go:35 TestPrepareTestFSDoesNotMountWithoutImportOrMarker ddeb5856cce052f0
#[test]
fn test_prepare_test_fs_does_not_mount_without_import_or_marker() {
    ets_go::testing::run(|t| {
        t.parallel();

        let mut testfs = testfs_of("/main.ts", "const value = 1\n");

        prepare_test_fs(&mut testfs);

        if testfs.contains_key("/.src/package.json") {
            t.fatal("expected no mounted Effect package without import or marker");
        }
        if testfs.contains_key("/node_modules/effect/package.json") {
            t.fatal("expected no mounted effect package without import or marker");
        }
    });
}

// go test: etstesthooks/init_test.go:52 assertMountedEffect c8048a8575315f02
fn assert_mounted_effect(t: &ets_go::testing::T, testfs: &TestFS) {
    t.helper();
    if !testfs.contains_key("/.src/package.json") {
        t.fatal("expected mounted /.src/package.json");
    }
    if !testfs.contains_key("/node_modules/effect/package.json") {
        t.fatal("expected mounted /node_modules/effect/package.json");
    }
}

// go test: etstesthooks/init_test.go:62 assertSourcePackageName 3127829e983612ec
fn assert_source_package_name(t: &ets_go::testing::T, testfs: &TestFS, want: &str) {
    t.helper();
    let Some(TestFSEntry::MapFile(file)) = testfs.get("/.src/package.json") else {
        t.fatal("expected /.src/package.json to be a fstest.MapFile");
    };
    let data = String::from_utf8_lossy(&file.data);
    if !data.contains(want) {
        t.fatal(format!(
            "expected /.src/package.json to contain {}, got {}",
            ets_go::strconv::quote(want),
            ets_go::strconv::quote(&data)
        ));
    }
}
