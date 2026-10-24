use std::sync::Arc;

use ets_go::sprintf;
use ets_shim::ast::{SourceFile, SourceFileParseOptions};
use ets_shim::core::{P, ScriptKind};
use ets_shim::parser;
use parking_lot::Mutex;
use rustc_hash::FxHashMap;

use crate::*;

// go test: internal/importstyle/barrel_test.go:14 fakeFiles
// fakeFiles serves loaded files as parsed program files and every file,
// loaded or not, through readFile, counting reads per file.
//
// Rust: shared with the reader's closures, so the maps sit behind an `Arc` and the counts behind a lock.
struct FakeFiles {
    loaded: FxHashMap<String, P<SourceFile>>,
    disk: FxHashMap<String, String>,
    reads: Mutex<FxHashMap<String, i64>>,
}

// go test: internal/importstyle/barrel_test.go:20 newFakeFiles
fn new_fake_files(loaded: &[(&str, &str)], disk: &[(&str, &str)]) -> Arc<FakeFiles> {
    let mut files = FakeFiles {
        loaded: FxHashMap::default(),
        disk: FxHashMap::default(),
        reads: Mutex::new(FxHashMap::default()),
    };
    for &(name, text) in loaded {
        let file_name = name.to_string();
        files.loaded.insert(
            file_name.clone(),
            parser::parse_source_file(
                SourceFileParseOptions { file_name: file_name.clone(), ..Default::default() },
                text,
                ScriptKind::TS,
            ),
        );
        files.disk.insert(file_name, text.to_string());
    }
    for &(name, text) in disk {
        files.disk.insert(name.to_string(), text.to_string());
    }
    Arc::new(files)
}

// go test: internal/importstyle/barrel_test.go:33 fakeFiles.reader
fn reader(f: &Arc<FakeFiles>) -> BarrelReader {
    let loaded = Arc::clone(f);
    let disk = Arc::clone(f);
    new_barrel_reader(
        move |file_name| loaded.loaded.get(file_name).copied(),
        move |file_name| {
            *disk.reads.lock().entry(file_name.to_string()).or_default() += 1;
            disk.disk.get(file_name).cloned()
        },
    )
}

// go test: internal/importstyle/barrel_test.go:44 TestIsBarrelFile
#[test]
fn test_is_barrel_file() {
    ets_go::testing::run(|t| {
        t.parallel();
        let tests: &[(&str, bool)] = &[
            ("/node_modules/effect/dist/index.d.ts", true),
            ("/node_modules/effect/dist/http/index.d.mts", true),
            ("/repo/src/index.ts", true),
            ("/node_modules/effect/dist/Array.d.ts", false),
            ("/node_modules/effect/dist/indexes.d.ts", false),
        ];
        for &(input, expected) in tests {
            let got = is_barrel_file(input);
            if got != expected {
                t.error(sprintf!("IsBarrelFile(%q) = %v, want %v", input, got, expected));
            }
        }
    });
}

// go test: internal/importstyle/barrel_test.go:63 TestBarrelReaderFind
#[test]
fn test_barrel_reader_find() {
    ets_go::testing::run(|t| {
        t.parallel();
        let files = new_fake_files(
            &[(
                "/node_modules/acme/dist/index.d.ts",
                r#"export * as Button from "./Button.js"; export * as Forms from "./forms/index.js"; export { helper } from "./helper.js""#,
            )],
            &[
                (
                    "/node_modules/acme/dist/forms/index.d.ts",
                    r#"export * as TextInput from "./TextInput.ts""#,
                ),
                (
                    "/node_modules/@scope/kit/lib/index.d.ts",
                    r#"export * as Dialog from "./Dialog""#,
                ),
                (
                    "/node_modules/renamed/dist/index.d.ts",
                    r#"export * as Http from "./HttpClient.js""#,
                ),
                (
                    "/node_modules/parent/dist/index.d.ts",
                    r#"export * as Deep from "./nested/Deep.js""#,
                ),
                ("/node_modules/parent/dist/nested/Deep.d.ts", "export declare const deep: number"),
            ],
        );
        let barrel = |specifier: &str, export_name: &str| Barrel {
            specifier: specifier.to_string(),
            export_name: export_name.to_string(),
        };
        let tests: Vec<(&str, &str, &str, Barrel, bool)> = vec![
            (
                "loaded root barrel",
                "acme/Button",
                "/node_modules/acme/dist/Button.d.ts",
                barrel("acme", "Button"),
                true,
            ),
            (
                "nested barrel read from disk",
                "acme/forms/TextInput",
                "/node_modules/acme/dist/forms/TextInput.d.ts",
                barrel("acme/forms", "TextInput"),
                true,
            ),
            (
                "scoped package read from disk",
                "@scope/kit/Dialog",
                "/node_modules/@scope/kit/lib/Dialog.d.ts",
                barrel("@scope/kit", "Dialog"),
                true,
            ),
            (
                "barrel export name differs from file",
                "renamed/HttpClient",
                "/node_modules/renamed/dist/HttpClient.d.ts",
                barrel("renamed", "Http"),
                true,
            ),
            (
                "forwarded from a deeper folder",
                "parent/nested/Deep",
                "/node_modules/parent/dist/nested/Deep.d.ts",
                barrel("parent", "Deep"),
                true,
            ),
            (
                "not forwarded by any barrel",
                "acme/forms/Select",
                "/node_modules/acme/dist/forms/Select.d.ts",
                Barrel::default(),
                false,
            ),
            (
                "named export only",
                "acme/helper",
                "/node_modules/acme/dist/helper.d.ts",
                Barrel::default(),
                false,
            ),
            (
                "module without a barrel above it",
                "acme/private/Secret",
                "/node_modules/acme/dist/private/Secret.d.ts",
                Barrel::default(),
                false,
            ),
            (
                "barrel itself",
                "acme/forms",
                "/node_modules/acme/dist/forms/index.d.ts",
                Barrel::default(),
                false,
            ),
            (
                "package root",
                "acme",
                "/node_modules/acme/dist/index.d.ts",
                Barrel::default(),
                false,
            ),
            (
                "relative specifier",
                "./Button",
                "/node_modules/acme/dist/Button.d.ts",
                Barrel::default(),
                false,
            ),
        ];
        let mut reader = reader(&files);
        for (name, specifier, module_file, expected, expected_found) in tests {
            let result = BarrelReader::find(Some(&mut reader), specifier, module_file);
            let found = result.is_some();
            let barrel = result.unwrap_or_default();
            if found != expected_found || barrel != expected {
                t.error(sprintf!(
                    "%s: Find(%q) = %+v, %v, want %+v, %v",
                    name,
                    specifier,
                    barrel,
                    found,
                    expected,
                    expected_found
                ));
            }
        }
    });
}

// go test: internal/importstyle/barrel_test.go:105 TestBarrelReaderReadsEachIndexOnce
#[test]
fn test_barrel_reader_reads_each_index_once() {
    ets_go::testing::run(|t| {
        t.parallel();
        let files = new_fake_files(
            &[],
            &[(
                "/node_modules/acme/dist/index.d.ts",
                r#"export * as Button from "./Button.js"; export * as Card from "./Card.js""#,
            )],
        );
        let mut reader = reader(&files);
        for module in ["Button", "Card", "Button"] {
            let found = BarrelReader::find(
                Some(&mut reader),
                &format!("acme/{module}"),
                &format!("/node_modules/acme/dist/{module}.d.ts"),
            )
            .is_some();
            if !found {
                t.fatal(sprintf!("Find(acme/%s) found no barrel", module));
            }
        }
        let got = files
            .reads
            .lock()
            .get("/node_modules/acme/dist/index.d.ts")
            .copied()
            .unwrap_or_default();
        if got != 1 {
            t.error(sprintf!("index.d.ts read %d times, want 1", got));
        }
    });
}
