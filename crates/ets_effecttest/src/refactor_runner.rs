use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use ets_bundledeffect::EffectVersion;
use ets_go::testing::T;
use ets_shim::core::tspath;
use ets_shim::ls::lsconv;
use ets_shim::lsproto;
use regex::Regex;

use crate::*;

// go test: internal/effecttest/refactor_runner.go:20 RefactorTestCasesDir 7e77ff4174589387
// RefactorTestCasesDir returns the path to the Effect refactor test cases directory.
pub fn refactor_test_cases_dir(version: EffectVersion) -> PathBuf {
    ets_bundledeffect::effect_ts_go_root_path()
        .join("testdata")
        .join("tests")
        .join(version.as_str().to_string() + "-refactors")
}

// go test: internal/effecttest/refactor_runner.go:25 DiscoverRefactorTestCases 0cc2f747507f9514
// DiscoverRefactorTestCases finds all .ts test files in the refactor test cases directory.
pub fn discover_refactor_test_cases(version: EffectVersion) -> std::io::Result<Vec<PathBuf>> {
    let dir = refactor_test_cases_dir(version);
    let entries = match read_dir_sorted(&dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err),
    };

    let mut cases = Vec::new();
    for (name, is_dir) in entries {
        if !is_dir && Path::new(&name).extension().is_some_and(|e| e == "ts") {
            cases.push(dir.join(name));
        }
    }
    Ok(cases)
}

// go test: internal/effecttest/refactor_runner.go:45 refactorCommentPrefix 70fa76e6d69f8d00
// refactorCommentPrefix matches lines starting with "// refactor:"
//
// Rust: Go's RE2 `\s` is an ASCII class; it is spelled out.
pub(crate) static REFACTOR_COMMENT_PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^//[\t\n\x0C\r ]*refactor:[\t\n\x0C\r ]*").unwrap());

// go test: internal/effecttest/refactor_runner.go:48 rangeRegex 994d13ec59297cd1
// rangeRegex matches a single "L:C-L:C" range token (dash-separated start and end).
pub(crate) static RANGE_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([0-9]+):([0-9]+)-([0-9]+):([0-9]+)$").unwrap());

// go test: internal/effecttest/refactor_runner.go:51 pointRegex d4c2465ac76d44ca
// pointRegex matches a single "L:C" point selection token (no dash).
pub(crate) static POINT_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([0-9]+):([0-9]+)$").unwrap());

// go test: internal/effecttest/refactor_runner.go:54 refactorCommentResult 12f0312a1042f14a
// refactorCommentResult holds the parsed refactor comment data.
#[derive(Clone, Debug, Default)]
pub(crate) struct RefactorCommentResult {
    pub(crate) found: bool,
    pub(crate) ranges: Vec<lsproto::Range>,
}

/// Rust-only: Go's `strconv.Atoi` with the error ignored (`n, _ := strconv.Atoi(s)`), then `uint32(n - 1)` (which
/// wraps for 0, as Go's conversion does).
fn one_based_to_u32(s: &str) -> u32 {
    let (n, _) = ets_go::strconv::atoi_value(s);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "Go's uint32(n - 1) conversion wraps"
    )]
    let v = n.wrapping_sub(1) as u32;
    v
}

// go test: internal/effecttest/refactor_runner.go:64 parseRefactorComment 4acf3c767c69f93e
// parseRefactorComment scans the test file content for a first-line
// "// refactor: L:C-L:C,L:C-L:C,..." comment (1-based lines and columns).
// Multiple ranges are separated by commas. A single "L:C" denotes a point
// selection (zero-width range where start == end). The comment is NOT stripped
// from the source content.
pub(crate) fn parse_refactor_comment(content: &str) -> RefactorCommentResult {
    let first_line = content.split('\n').next().unwrap_or_default();
    let first_line = ets_go::strings::trim_space(first_line);

    if !REFACTOR_COMMENT_PREFIX.is_match(first_line) {
        return RefactorCommentResult { found: false, ..Default::default() };
    }

    // Strip the "// refactor:" prefix to get the ranges portion
    let payload = REFACTOR_COMMENT_PREFIX.replace_all(first_line, "");

    // Split on "," to get individual range/point tokens
    let mut ranges = Vec::new();
    for part in payload.split(',') {
        let part = ets_go::strings::trim_space(part);
        if part.is_empty() {
            continue;
        }
        if let Some(m) = RANGE_REGEX.captures(part) {
            ranges.push(lsproto::Range {
                start: lsproto::Position {
                    line: one_based_to_u32(&m[1]),
                    character: one_based_to_u32(&m[2]),
                },
                end: lsproto::Position {
                    line: one_based_to_u32(&m[3]),
                    character: one_based_to_u32(&m[4]),
                },
            });
        } else if let Some(m) = POINT_REGEX.captures(part) {
            let pos = lsproto::Position {
                line: one_based_to_u32(&m[1]),
                character: one_based_to_u32(&m[2]),
            };
            ranges.push(lsproto::Range { start: pos, end: pos });
        }
    }

    if ranges.is_empty() {
        return RefactorCommentResult { found: false, ..Default::default() };
    }

    RefactorCommentResult { found: true, ranges }
}

// go test: internal/effecttest/refactor_runner.go:115 RunEffectRefactorTest 6bce5f171307652c
// RunEffectRefactorTest executes a single Effect refactor baseline test case.
// It creates a fourslash test instance, collects refactor inventory and application
// results for the selection specified by a "// refactor:" comment, and generates a *.refactors.txt baseline.
//
// Rust: the fourslash harness is the Rust-only stand-in `ets_fourslash` (see `run_effect_quick_fix_test`).
pub fn run_effect_refactor_test(t: &T, version: EffectVersion, test_file: &Path) {
    init_hooks();
    acquire_program();
    let _release = ReleaseProgramOnDrop;

    // Read the test file
    let content = match std::fs::read_to_string(test_file) {
        Ok(content) => content,
        Err(err) => t.fatal(format!("Failed to read test file: {err}")),
    };

    // Parse the "// refactor:" comment marker (comment is preserved in source)
    let rc = parse_refactor_comment(&content);
    let source_content = content.as_str();

    // Parse test file into units (handles @filename directives)
    let default_file_name = tspath::get_base_file_name(&test_file.to_string_lossy());
    let units = parse_test_units(source_content, &default_file_name);

    // Build fourslash content with @filename directives
    let mut sb = String::new();
    let current_directory = "/.src";

    // Check if a tsconfig is provided
    let mut has_ts_config = false;
    for unit in &units {
        let unit_name = tspath::get_normalized_absolute_path(&unit.name, current_directory);
        if ets_go::strings::to_lower(&unit_name).ends_with("tsconfig.json")
            || ets_go::strings::to_lower(&unit_name).ends_with("jsconfig.json")
        {
            has_ts_config = true;
            break;
        }
    }

    // Inject default tsconfig if none was provided
    if !has_ts_config {
        sb.push_str("// @filename: ");
        sb.push_str(&tspath::get_normalized_absolute_path("tsconfig.json", current_directory));
        sb.push('\n');
        sb.push_str(DEFAULT_TS_CONFIG);
        sb.push('\n');
    }

    // Collect test file paths for later use
    let mut test_file_names: Vec<String> = Vec::new();

    // Add all test units as fourslash files
    for unit in &units {
        let unit_name = tspath::get_normalized_absolute_path(&unit.name, current_directory);
        sb.push_str("// @filename: ");
        sb.push_str(&unit_name);
        sb.push('\n');
        sb.push_str(&unit.content);
        sb.push('\n');

        // Track non-config files
        let lower = ets_go::strings::to_lower(&unit_name);
        if !lower.ends_with("tsconfig.json") && !lower.ends_with("jsconfig.json") {
            test_file_names.push(unit_name);
        }
    }

    // Create fourslash test instance
    let mut f = ets_fourslash::new_fourslash(t, None, &sb);

    // Collect inventory and application results
    let mut inventory: Vec<RefactorInventoryEntry> = Vec::new();
    let mut results: Vec<RefactorApplicationResult> = Vec::new();
    let mut range_counter = 0;

    // If a "// refactor:" comment was found, test with each parsed selection range
    if rc.found && !test_file_names.is_empty() {
        let file_uri = lsconv::file_name_to_document_uri(&test_file_names[0]);

        for sel_range in &rc.ranges {
            range_counter += 1;
            let range_id = format!("R{range_counter}");

            let actions = f.get_refactor_actions_for_range(t, &file_uri, *sel_range);

            let mut titles = Vec::new();
            let mut kinds = Vec::new();
            for action in &actions {
                titles.push(action.title.clone());
                kinds.push(action.kind.clone());
            }

            let range_desc = format!(
                "{}:{}-{}:{}",
                sel_range.start.line.wrapping_add(1),
                sel_range.start.character.wrapping_add(1),
                sel_range.end.line.wrapping_add(1),
                sel_range.end.character.wrapping_add(1),
            );

            inventory.push(RefactorInventoryEntry {
                id: range_id.clone(),
                range_text: range_desc,
                action_titles: titles,
                action_kinds: kinds,
            });

            // Apply each refactor action and collect results
            for action_idx in 0..actions.len() {
                // Reset caret to beginning of file before each action to avoid
                // stale caret positions from a previous action's apply+undo cycle.
                f.go_to_bof(t);
                let result = f.apply_refactor_action(t, &file_uri, *sel_range, action_idx);
                results.push(RefactorApplicationResult {
                    range_id: range_id.clone(),
                    action_index: result.action_index,
                    action_title: result.action_title,
                    changes: result.changes,
                });
            }
        }
    }

    // Also test that an empty selection (zero-width range) produces no refactors
    if !test_file_names.is_empty() {
        let file_uri = lsconv::file_name_to_document_uri(&test_file_names[0]);
        let empty_range = ets_fourslash::RangeMarker::default();
        let _empty_actions = f.get_refactor_actions_for_range(t, &file_uri, empty_range.ls_range);

        range_counter += 1;
        let range_id = format!("R{range_counter}");
        inventory.push(RefactorInventoryEntry {
            id: range_id,
            range_text: "empty (0:0-0:0)".to_string(),
            action_titles: Vec::new(),
            action_kinds: Vec::new(),
        });
    }

    // Generate baseline
    let base_file_name = tspath::get_base_file_name(&test_file.to_string_lossy());
    let baseline_name = base_file_name.strip_suffix(".ts").unwrap_or(&base_file_name).to_string();
    let baseline_subfolder = version.as_str();
    do_refactor_baseline(t, &baseline_name, baseline_subfolder, &inventory, &results);

    f.done(t);
}
