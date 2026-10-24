use std::path::PathBuf;

use ets_shim::core::context::Context;
use serde_json::Value;

use crate::*;

// Rust: the fixtures are read in place from etsdiagnostics/testdata. Go's test package imports etscheckerhooks,
// whose `init()` registers the Effect checker hooks: `run_request` calls `ets_etscheckerhooks::init()`. The tests
// that need rules not ported yet wait for them (see their `#[ignore]`).

/// Rust-only: Go's `filepath.Abs("testdata/<name>")` in the etsdiagnostics package directory.
fn testdata_dir(name: &str) -> String {
    ets_bundledeffect::effect_ts_go_root_path()
        .join("etsdiagnostics/testdata")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

/// Rust-only: Go's `json.Marshal(request{...})` (every field without `omitempty` is written).
fn marshal_request(req: &Request) -> String {
    let mut object = serde_json::Map::new();
    object.insert("cwd".into(), Value::String(req.cwd.clone()));
    if !req.file.is_empty() {
        object.insert("file".into(), Value::String(req.file.clone()));
    }
    if !req.project.is_empty() {
        object.insert("project".into(), Value::String(req.project.clone()));
    }
    object.insert("format".into(), Value::String(req.format.clone()));
    object.insert("strict".into(), Value::Bool(req.strict));
    if !req.severity.is_empty() {
        object.insert("severity".into(), Value::String(req.severity.clone()));
    }
    object.insert("progress".into(), Value::Bool(req.progress));
    if let Some(lsp_config) = &req.lsp_config {
        object.insert("lspconfig".into(), Value::String(lsp_config.clone()));
    }
    if req.list_files {
        object.insert("listFiles".into(), Value::Bool(true));
    }
    Value::Object(object).to_string()
}

/// Rust-only: `Run` with byte buffers for stdout and stderr.
fn run_request(request: &str) -> (i32, String, String) {
    ets_etscheckerhooks::init();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run(&Context::background(), &[request.to_string()], &mut stdout, &mut stderr);
    (status, String::from_utf8(stdout).unwrap(), String::from_utf8(stderr).unwrap())
}

// go test: etsdiagnostics/diagnostics_test.go:19 TestParseSeverityFilter
#[test]
fn test_parse_severity_filter() {
    let filter = parse_severity_filter(" ERROR, warning,invalid ").expect("a filter");
    assert!(filter.contains(&Severity::Error), "expected error severity");
    assert!(filter.contains(&Severity::Warning), "expected warning severity");
    assert!(!filter.contains(&Severity::Message), "did not expect message severity");
    assert!(
        parse_severity_filter("invalid").is_none(),
        "an entirely invalid filter should disable filtering"
    );
}

// go test: etsdiagnostics/diagnostics_test.go:37 TestParseLSPConfigPresence
#[test]
fn test_parse_lsp_config_presence() {
    let result = parse_lsp_config(None);
    assert!(
        matches!(result, Ok((None, false))),
        "unexpected omitted config result: {:?}",
        result.as_ref().map(|(config, provided)| (config.is_some(), *provided))
    );
    let result = parse_lsp_config(Some(""));
    assert!(result.is_err(), "expected an explicitly empty config to fail");
    let result = parse_lsp_config(Some("null"));
    assert!(
        matches!(result, Ok((Some(_), true))),
        "unexpected null config result: {:?}",
        result.as_ref().map(|(config, provided)| (config.is_some(), *provided))
    );
}

fn diagnostic(
    location: Location,
    severity: Severity,
    code: i32,
    name: &str,
    message: &str,
) -> FormattedDiagnostic {
    FormattedDiagnostic {
        location,
        severity,
        code,
        name: name.to_string(),
        message: message.to_string(),
        related_information: Vec::new(),
    }
}

// go test: etsdiagnostics/diagnostics_test.go:53 TestWriteJSONOutput
#[test]
fn test_write_json_output() {
    let diagnostics = [diagnostic(
        Location {
            file: "/workspace/main.ts".to_string(),
            start: 2,
            length: 4,
            line: 1,
            column: 3,
            end_line: 1,
            end_column: 7,
            source: "",
        },
        Severity::Error,
        377_001,
        "floatingEffect",
        "Effect must be handled",
    )];
    let want_summary =
        Summary { files_checked: 1, total_files: 1, errors: 1, ..Default::default() };
    let mut output = Vec::new();
    write_output(&mut output, "json", &diagnostics, &[], want_summary).unwrap();
    let output = String::from_utf8(output).unwrap();
    for expected in [
        r#""file": "/workspace/main.ts""#,
        r#""severity": "error""#,
        r#""name": "floatingEffect""#,
        r#""filesChecked": 1"#,
    ] {
        assert!(output.contains(expected), "output did not contain {expected:?}:\n{output}");
    }
}

// go test: etsdiagnostics/diagnostics_test.go:78 TestWriteGitHubActionsOutput
#[test]
fn test_write_git_hub_actions_output() {
    let diagnostics = [diagnostic(
        Location {
            file: "/workspace/main.ts".to_string(),
            line: 2,
            column: 3,
            end_line: 2,
            end_column: 8,
            ..Default::default()
        },
        Severity::Message,
        0,
        "floatingEffect",
        "first%\nsecond",
    )];
    let mut output = Vec::new();
    write_output(
        &mut output,
        "github-actions",
        &diagnostics,
        &[],
        Summary { files_checked: 1, total_files: 1, messages: 1, ..Default::default() },
    )
    .unwrap();
    let output = String::from_utf8(output).unwrap();
    let want = "::notice file=/workspace/main.ts,line=2,col=3,endLine=2,endColumn=8,title=floatingEffect::first%25%0Asecond\n";
    assert!(output.starts_with(want), "unexpected output:\n{output}");
}

// go test: etsdiagnostics/diagnostics_test.go:95 TestWriteFilesBlock
#[test]
fn test_write_files_block() {
    let files = [FileEffectVersion {
        file: "/workspace/main.ts".to_string(),
        detected_effect: "unknown".to_string(),
        supported_effect: "v3".to_string(),
    }];
    let mut output = Vec::new();
    write_output(
        &mut output,
        "text",
        &[],
        &files,
        Summary { total_files: 1, files_checked: 1, ..Default::default() },
    )
    .unwrap();
    let output = String::from_utf8(output).unwrap();
    assert!(
        output.contains("/workspace/main.ts: detected=unknown, supported=v3"),
        "output missing per-file version block:\n{output}"
    );
}

// go test: etsdiagnostics/diagnostics_test.go:110 TestRunJSON
#[test]
fn test_run_json() {
    let cwd = testdata_dir("native-diagnostics");
    for (name, mut req) in [
        (
            "project",
            Request {
                project: "tsconfig.json".into(),
                format: "json".into(),
                ..Default::default()
            },
        ),
        ("file", Request { file: "main.ts".into(), format: "json".into(), ..Default::default() }),
    ] {
        req.cwd.clone_from(&cwd);
        let (status, stdout, stderr) = run_request(&marshal_request(&req));
        assert_eq!(status, 1, "{name}: unexpected status {status}; stderr:\n{stderr}");
        let output: Value = serde_json::from_str(&stdout)
            .unwrap_or_else(|err| panic!("{name}: invalid JSON output: {err}\n{stdout}"));
        let diagnostics = output["diagnostics"].as_array().expect("diagnostics");
        assert_eq!(
            diagnostics.len(),
            1,
            "{name}: expected one diagnostic, got {}",
            diagnostics.len()
        );
        let diagnostic = &diagnostics[0];
        assert!(
            diagnostic["name"] == "asyncFunction"
                && diagnostic["code"] == 377_081
                && diagnostic["severity"] == "error",
            "{name}: unexpected diagnostic: {diagnostic}"
        );
        let message = diagnostic["message"].as_str().unwrap_or_default();
        assert!(
            !message.contains("effect(asyncFunction)"),
            "{name}: message includes redundant rule name: {message:?}"
        );
        assert!(
            output.get("files").is_none_or(Value::is_null),
            "{name}: expected no files list without listFiles: {}",
            output["files"]
        );
    }
}

// go test: etsdiagnostics/diagnostics_test.go:154 TestRunProjectJSONListFiles
#[test]
fn test_run_project_json_list_files() {
    let req = Request {
        cwd: testdata_dir("native-diagnostics"),
        project: "tsconfig.json".into(),
        format: "json".into(),
        list_files: true,
        ..Default::default()
    };
    let (status, stdout, stderr) = run_request(&marshal_request(&req));
    assert_eq!(status, 1, "unexpected status {status}; stderr:\n{stderr}");
    let output: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|err| panic!("invalid JSON output: {err}\n{stdout}"));
    let files = output["files"].as_array().cloned().unwrap_or_default();
    assert!(!files.is_empty(), "expected at least one file entry:\n{stdout}");
    for entry in &files {
        let file = entry["file"].as_str().unwrap_or_default();
        assert!(!file.is_empty(), "empty file name in entry: {entry}");
        let supported = entry["supportedEffect"].as_str().unwrap_or_default();
        assert!(
            supported == "v3" || supported == "v4",
            "unexpected supported version {supported:?} for {file}"
        );
        assert!(
            !entry["detectedEffect"].as_str().unwrap_or_default().is_empty(),
            "empty detected version for {file}"
        );
    }
}

// go test: etsdiagnostics/diagnostics_test.go:192 missingEffectContextSource
const MISSING_EFFECT_CONTEXT_SOURCE: &str = r#"import { Context, Effect, Layer } from "effect"

class Db extends Context.Service<Db>()("Db", { make: Effect.succeed({}) }) {
  static Default = Layer.effect(this, this.make)
}

class Fs extends Context.Service<Fs>()("Fs", { make: Effect.succeed({}) }) {
  static Default = Layer.effect(this, this.make)
}

class Repo extends Context.Service<Repo>()("Repo", { make: Effect.as(Effect.andThen(Db, Fs), {}) }) {
  static Default = Layer.effect(this, this.make)
}

const live = Repo.Default.pipe(Layer.provide(Fs.Default))

Effect.gen(function*() {
  yield* Fs
  yield* Db
}).pipe(
  Effect.provide(live),
  Effect.runPromise
)
"#;

/// Rust-only: Go's `t.TempDir()`, removed when dropped.
struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// go test: etsdiagnostics/diagnostics_test.go:217 writeEffectV4Project
// Rust: returns the temporary directory too (Go's `t.TempDir` is removed by the test framework).
fn write_effect_v4_project(source: &str) -> (TempDir, String) {
    let mut files = ets_bundledeffect::TestFS::default();
    if let Err(err) =
        ets_bundledeffect::mount_effect(ets_bundledeffect::EffectVersion::V4, &mut files)
    {
        panic!("Effect not installed: {err}");
    }
    let root = std::env::temp_dir().join(format!(
        "ets_etsdiagnostics-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let root = TempDir(root);
    files.insert(
        "/.src/tsconfig.json".to_string(),
        ets_shim::vfs::vfstest::MapFile::from(
            r#"{
  "compilerOptions": {
    "strict": true,
    "skipLibCheck": true,
    "plugins": [{ "name": "@effect/language-service" }]
  },
  "files": ["main.ts"]
}"#,
        ),
    );
    files.insert("/.src/main.ts".to_string(), ets_shim::vfs::vfstest::MapFile::from(source));
    for (name, file) in &files {
        let path = root.0.join(name.trim_start_matches('/'));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &file.data).unwrap();
    }
    let cwd = root.0.join(".src").to_string_lossy().into_owned();
    (root, cwd)
}

// go test: etsdiagnostics/diagnostics_test.go:245 runMissingEffectContext
fn run_missing_effect_context(format: &str) -> (TempDir, String, String) {
    let (root, cwd) = write_effect_v4_project(MISSING_EFFECT_CONTEXT_SOURCE);
    let req = Request {
        cwd: cwd.clone(),
        project: "tsconfig.json".into(),
        format: format.into(),
        ..Default::default()
    };
    let (status, stdout, stderr) = run_request(&marshal_request(&req));
    assert_eq!(status, 1, "unexpected status {status}; stderr:\n{stderr}");
    (root, stdout, PathBuf::from(cwd).join("main.ts").to_string_lossy().into_owned())
}

// go test: etsdiagnostics/diagnostics_test.go:260 TestRunJSONReportsRelatedInformation
#[test]
#[ignore = "needs the missingEffectContext rule (Phase 4)"]
fn test_run_json_reports_related_information() {
    let (_root, stdout, file) = run_missing_effect_context("json");
    let output: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|err| panic!("invalid JSON output: {err}\n{stdout}"));
    let diagnostics = output["diagnostics"].as_array().cloned().unwrap_or_default();
    assert!(
        diagnostics.len() == 1 && diagnostics[0]["code"] == 377_004,
        "expected one missingEffectContext diagnostic:\n{stdout}"
    );
    let want = serde_json::json!([
        {
            "file": file, "start": 580, "length": 14,
            "line": 21, "column": 3, "endLine": 21, "endColumn": 17,
            "code": 377_059, "message": "Adjusting this layer composition could provide the missing service.",
        },
        {
            "file": file, "start": 264, "length": 7,
            "line": 8, "column": 10, "endLine": 8, "endColumn": 17,
            "code": 377_060, "message": "This layer provides the missing service Fs.",
        },
    ]);
    let got = &diagnostics[0]["relatedInformation"];
    assert_eq!(got, &want, "unexpected relatedInformation");
}

// go test: etsdiagnostics/diagnostics_test.go:294 TestRunPrettyReportsRelatedInformation
#[test]
#[ignore = "needs the missingEffectContext rule (Phase 4)"]
fn test_run_pretty_reports_related_information() {
    let (_root, stdout, file) = run_missing_effect_context("pretty");
    let plain = regex::Regex::new("\x1b\\[[0-9;]*m").unwrap().replace_all(&stdout, "");
    let want = file.clone()
        + ":22:3 - error effect(missingEffectContext): This Effect requires a service that is missing from the expected Effect context: `Db | Fs`.\n"
        + "\n"
        + "22   Effect.runPromise\n"
        + "     ~~~~~~~~~~~~~~~~~\n"
        + "\n"
        + "  "
        + &file
        + ":21:3 - Adjusting this layer composition could provide the missing service.\n"
        + "    21   Effect.provide(live),\n"
        + "         ~~~~~~~~~~~~~~\n"
        + "\n"
        + "  "
        + &file
        + ":8:10 - This layer provides the missing service Fs.\n"
        + "    8   static Default = Layer.effect(this, this.make)\n"
        + "               ~~~~~~~\n"
        + "\n"
        + "Checked 1 files out of 1 files. \n";
    assert!(
        plain.starts_with(&want),
        "unexpected pretty output\ngot:\n{plain}\nwant prefix:\n{want}"
    );
}

/// Rust-only: the Go-shaped JSON encoder (`encode_json_output`) writes what Go's `json.Encoder` with
/// `SetIndent("", "  ")` writes, including HTML escaping and an omitted empty `files` / `relatedInformation`.
#[test]
fn json_output_matches_go_encoder() {
    let diagnostics = [FormattedDiagnostic {
        related_information: vec![RelatedInformation {
            location: Location { file: "/a.ts".into(), line: 1, column: 1, ..Default::default() },
            code: 377_060,
            message: "a <b> & \"c\"".into(),
        }],
        ..diagnostic(
            Location { file: "/a.ts".into(), ..Default::default() },
            Severity::Warning,
            377_004,
            "x",
            "m\n",
        )
    }];
    let got = encode_json_output(&JsonOutput {
        diagnostics: &diagnostics,
        files: &[],
        summary: Summary::default(),
    });
    let want = r#"{
  "diagnostics": [
    {
      "file": "/a.ts",
      "start": 0,
      "length": 0,
      "line": 0,
      "column": 0,
      "endLine": 0,
      "endColumn": 0,
      "severity": "warning",
      "code": 377004,
      "name": "x",
      "message": "m\n",
      "relatedInformation": [
        {
          "file": "/a.ts",
          "start": 0,
          "length": 0,
          "line": 1,
          "column": 1,
          "endLine": 0,
          "endColumn": 0,
          "code": 377060,
          "message": "a \u003cb\u003e \u0026 \"c\""
        }
      ]
    }
  ],
  "summary": {
    "filesChecked": 0,
    "filesWithoutPlugin": 0,
    "totalFiles": 0,
    "errors": 0,
    "warnings": 0,
    "messages": 0
  }
}
"#;
    assert_eq!(got, want);
}

/// Rust-only: the request decoder follows Go's `encoding/json` rules (case-insensitive keys, `null` ignored, type
/// errors), and `Run` reports the plugin-less project of the `without-plugin` fixture without hooks.
#[test]
fn decode_request_follows_go_rules() {
    let req = decode_request(r#"{"CWD":"/x","format":null,"listfiles":true,"other":1}"#).unwrap();
    assert_eq!(req.cwd, "/x");
    assert!(req.list_files);
    assert_eq!(
        decode_request(r#"{"cwd":1}"#).unwrap_err(),
        "json: cannot unmarshal number into Go struct field request.cwd of type string"
    );
    assert_eq!(
        decode_request("[]").unwrap_err(),
        "json: cannot unmarshal array into Go value of type etsdiagnostics.request"
    );
    assert_eq!(decode_request("null").unwrap(), Request::default());
}

/// Rust-only: an end-to-end run that needs no Effect hooks: the `without-plugin` fixture is skipped.
#[test]
fn run_reports_files_without_plugin() {
    let req = Request {
        cwd: testdata_dir("without-plugin"),
        project: "tsconfig.json".into(),
        format: "text".into(),
        ..Default::default()
    };
    let (status, stdout, stderr) = run_request(&marshal_request(&req));
    assert_eq!(status, 0, "unexpected status {status}; stderr:\n{stderr}");
    assert_eq!(
        stdout,
        "Checked 0 files out of 1 files. \nSkipped 1 files because their tsconfig does not enable the @effect/language-service plugin.\n0 errors, 0 warnings and 0 messages.\n"
    );
    let (status, _, stderr) = run_request(r#"{"format":"json"}"#);
    assert_eq!(status, 2);
    assert_eq!(stderr, "diagnostics request is missing cwd\n");
}
