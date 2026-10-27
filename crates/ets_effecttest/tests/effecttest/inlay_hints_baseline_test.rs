use ets_effecttest::baseline_results::{self, Outcome};
use ets_fourslash::FourslashTest;
use ets_go::testing::T;
use ets_shim::core::json;
use ets_shim::ls::lsconv;
use ets_shim::ls::lsutil::{self, UserPreferences};
use ets_shim::lsproto::{self, Json};

// go test: internal/effecttest/inlay_hints_baseline_test.go:22 verifyLocalBaselineInlayHints c252efbfb50b136e
//
// Rust: Go builds the request message by hand through the shim accessor `fourslash.FourslashTest_client` to send it
// without fourslash's checks; `LSPClient::send_request` is the same request. The deferred reset is run at the end.
pub(crate) fn verify_local_baseline_inlay_hints(
    t: &T,
    f: &mut FourslashTest,
    test_content: &str,
    file_name: &str,
    preferences: Option<UserPreferences>,
) {
    t.helper();

    let preferences = preferences.unwrap_or_else(lsutil::new_default_user_preferences);
    let reset = f.configure_with_reset(t, preferences);

    let file_content = must_find_fourslash_file_content(t, test_content, file_name);
    let line_map = lsconv::compute_lsp_line_starts(&file_content);
    let last_line_start = *line_map.line_starts.last().unwrap();
    let end = lsproto::Position {
        line: u32::try_from(line_map.line_starts.len() - 1).unwrap(),
        // Go adds `utf8.RuneLen(r)` for every rune of the last line: its length in bytes.
        character: u32::try_from(file_content[last_line_start as usize..].len()).unwrap(),
    };

    let params = lsproto::InlayHintParams {
        text_document: lsproto::TextDocumentIdentifier {
            uri: lsconv::file_name_to_document_uri(file_name),
        },
        range: lsproto::Range { start: lsproto::Position { line: 0, character: 0 }, end },
        ..Default::default()
    };

    let (resp, result) = f.client().send_request(lsproto::TEXT_DOCUMENT_INLAY_HINT_INFO, params);
    if let Some(error) = &resp.error {
        t.fatal(format!(
            "{} request returned error: {}",
            lsproto::TEXT_DOCUMENT_INLAY_HINT_INFO.method.0,
            error.message
        ));
    }

    let Some(result) = result else {
        t.fatal(format!(
            "Failed to unmarshal {} response",
            lsproto::TEXT_DOCUMENT_INLAY_HINT_INFO.method.0
        ));
    };

    let actual = format_inlay_hint_baseline(t, &file_content, result);
    run_local_inlay_baseline(t, &local_inlay_baseline_file_name(t), &actual);

    f.configure(t, reset);
}

// go test: internal/effecttest/inlay_hints_baseline_test.go:69 mustFindFourslashFileContent 61e0b060c004f626
pub(crate) fn must_find_fourslash_file_content(
    t: &T,
    test_content: &str,
    file_name: &str,
) -> String {
    t.helper();

    let parsed =
        ets_fourslash::parse_test_data(test_content, &(local_inlay_baseline_file_name(t) + ".ts"));
    for file in &parsed.files {
        if file.file_name() == file_name {
            return file.content.clone();
        }
    }
    t.fatal(format!("fourslash file {file_name} not found"));
}

// go test: internal/effecttest/inlay_hints_baseline_test.go:82 formatInlayHintBaseline 3027fe879d03113e
pub(crate) fn format_inlay_hint_baseline(
    t: &T,
    file_content: &str,
    mut result: lsproto::InlayHintResponse,
) -> String {
    t.helper();

    let file_lines: Vec<&str> = file_content.split('\n').collect();
    let mut annotations: Vec<String> = Vec::new();
    if let Some(inlay_hints) = &mut result.inlay_hints {
        ets_go::slices::sort_func(inlay_hints, |a, b| {
            lsproto::compare_positions(a.position, b.position)
        });
        for hint in inlay_hints.iter_mut() {
            if let Some(parts) = &mut hint.label.inlay_hint_label_parts {
                for part in parts {
                    if let Some(location) = &mut part.location {
                        if is_lib_file(&location.uri.file_name()) {
                            location.range.start = lsproto::Position { line: 0, character: 0 };
                            location.range.end = lsproto::Position { line: 0, character: 0 };
                        }
                    }
                }
            }
            let underline = " ".repeat(hint.position.character as usize) + "^";
            let hint_json = match json::marshal_indent(&hint.to_json(), "", "  ") {
                Ok(hint_json) => hint_json,
                Err(err) => t.fatal(format!("Failed to stringify inlay hint for baseline: {err}")),
            };
            let mut annotation = file_lines[hint.position.line as usize].to_string();
            annotation += &("\n".to_string() + &underline + "\n" + &hint_json);
            annotations.push(annotation);
        }
    }

    if annotations.is_empty() {
        annotations.push("=== No inlay hints ===".to_string());
    }

    annotations.join("\n\n")
}

// go test: internal/effecttest/inlay_hints_baseline_test.go:118 runLocalInlayBaseline 92ca0e7911f587ff
//
// Rust: as `run_effect_baseline`: the Go runner owns the reference, so a missing reference is a mismatch here (Go
// creates it); the outcome is recorded by `baseline_results::report`, which fails a mismatch as Go does.
pub(crate) fn run_local_inlay_baseline(t: &T, file_name: &str, actual: &str) {
    t.helper();

    let local_path = ets_effecttest::baseline_local_path("inlayHints").join(file_name);
    let reference_path = ets_effecttest::baseline_reference_path("inlayHints").join(file_name);
    let baseline = format!("inlayHints/{file_name}");
    let expected = match std::fs::read(&reference_path) {
        Ok(reference_content) => Some(String::from_utf8_lossy(&reference_content).into_owned()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => t.fatal(format!("Failed to read reference baseline: {err}")),
    };

    if expected.as_deref() == Some(actual) {
        baseline_results::report(t, &baseline, Outcome::Match, String::new);
        return;
    }
    if let Err(err) = std::fs::create_dir_all(local_path.parent().unwrap()) {
        t.fatal(format!("Failed to create local baseline directory: {err}"));
    }
    if let Err(err) = std::fs::write(&local_path, actual) {
        t.fatal(format!("Failed to write local baseline: {err}"));
    }
    baseline_results::report(t, &baseline, Outcome::Mismatch, || {
        let diff = ets_effecttest::diff_text(
            &reference_path.to_string_lossy(),
            &local_path.to_string_lossy(),
            expected.as_deref().unwrap_or_default(),
            actual,
        );
        let diff_lines: Vec<String> = diff.split('\n').map(|line| format!("  {line}")).collect();
        format!("Baseline mismatch:\n{}", diff_lines.join("\n"))
    });
}

// go test: internal/effecttest/inlay_hints_baseline_test.go:161 localInlayBaselineFileName 1ac295f024691c49
pub(crate) fn local_inlay_baseline_file_name(t: &T) -> String {
    t.helper();
    let name = t.name();
    let name = name.rsplit('/').next().unwrap_or_default();
    let name = name.strip_prefix("Test").unwrap_or(name);
    if name.is_empty() {
        return "inlayHints.baseline".to_string();
    }
    let mut chars = name.chars();
    let r = chars.next().unwrap();
    ets_go::strings::to_lower(&r.to_string()) + chars.as_str() + ".baseline"
}

// go test: internal/effecttest/inlay_hints_baseline_test.go:175 isLibFile ce0ac1aaa5a55871
pub(crate) fn is_lib_file(file_name: &str) -> bool {
    let base_name = std::path::Path::new(file_name)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    base_name.starts_with("lib.") && base_name.ends_with(".d.ts")
}
