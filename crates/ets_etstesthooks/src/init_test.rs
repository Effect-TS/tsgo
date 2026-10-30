use ets_fourslash::{TestFS, TestFSEntry};

use crate::init::*;

/// Rust-only: Go's `map[string]any{path: content}` literal.
fn testfs_of(path: &str, content: &str) -> TestFS {
    let mut testfs = TestFS::default();
    testfs.insert(path.to_string(), TestFSEntry::String(content.to_string()));
    testfs
}

// go test: etstesthooks/init_test.go:9 TestPrepareTestFSMountsEffect ba6e93c683c714a5
#[test]
fn test_prepare_test_fs_mounts_effect() {
    ets_go::testing::run(|t| {
        t.parallel();

        struct Case {
            name: &'static str,
            source: &'static str,
            mounted: bool,
            package_name: &'static str,
        }
        for tt in [
            Case {
                name: "v4 marker",
                source: "// @effect-v4\nconst value = 1\n",
                mounted: true,
                package_name: "effect-v4-tests",
            },
            Case {
                name: "v3 marker",
                source: "// @effect-v3\nconst value = 1\n",
                mounted: true,
                package_name: "effect-v3-tests",
            },
            Case {
                name: "no import or marker",
                source: "const value = 1\n",
                mounted: false,
                package_name: "",
            },
        ] {
            t.run(tt.name, |t| {
                t.parallel();
                let mut testfs = testfs_of("/main.ts", tt.source);

                prepare_test_fs(&mut testfs);

                let source_package = testfs.get("/.src/package.json");
                let has_source_package = source_package.is_some();
                let has_effect_package = testfs.contains_key("/node_modules/effect/package.json");
                if has_source_package != tt.mounted || has_effect_package != tt.mounted {
                    t.fatal(format!(
                        "mounted /.src/package.json={has_source_package} /node_modules/effect/package.json={has_effect_package}, want {}",
                        tt.mounted
                    ));
                }
                if !tt.mounted {
                    return;
                }
                let Some(TestFSEntry::MapFile(file)) = source_package else {
                    t.fatal("expected /.src/package.json to be a fstest.MapFile");
                };
                let data = String::from_utf8_lossy(&file.data);
                if !data.contains(tt.package_name) {
                    t.fatal(format!(
                        "expected /.src/package.json to contain {}, got {}",
                        ets_go::strconv::quote(tt.package_name),
                        ets_go::strconv::quote(&data)
                    ));
                }
            });
        }
    });
}
