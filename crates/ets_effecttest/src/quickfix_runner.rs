use std::path::Path;

use ets_bundledeffect::EffectVersion;
use ets_go::testing::T;
use ets_shim::core::tspath;
use ets_shim::ls::lsconv;

use crate::*;

// go test: internal/effecttest/quickfix_runner.go:17 isDisableStyleFix
// isDisableStyleFix returns true if the fix title matches the disable-style pattern:
// "Disable ... for this line" or "Disable ... for entire file".
pub(crate) fn is_disable_style_fix(title: &str) -> bool {
    title.starts_with("Disable ")
        && (title.ends_with(" for this line") || title.ends_with(" for entire file"))
}

// go test: internal/effecttest/quickfix_runner.go:25 RunEffectQuickFixTest
// RunEffectQuickFixTest executes a single Effect quick-fix baseline test case.
// It creates a fourslash test instance, collects quick-fix inventory and application
// results, and generates a *.quickfixes.txt baseline.
//
// Rust: the fourslash harness is the Rust-only stand-in `ets_fourslash` (a tsrs language server driven through
// tsrs's in-process LSP client, as Go's drives typescript-go's); `done` runs when the test ends (also on drop).
pub fn run_effect_quick_fix_test(t: &T, version: EffectVersion, test_file: &Path) {
    init_hooks();
    acquire_program();
    let _release = ReleaseProgramOnDrop;

    // Read the test file
    let content = match std::fs::read_to_string(test_file) {
        Ok(content) => content,
        Err(err) => t.fatal(format!("Failed to read test file: {err}")),
    };

    // Parse test file into units (handles @filename directives)
    let default_file_name = tspath::get_base_file_name(&test_file.to_string_lossy());
    let units = parse_test_units(&content, &default_file_name);

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

    // Collect inventory and application results across all test files
    let mut inventory: Vec<QuickFixInventoryEntry> = Vec::new();
    let mut results: Vec<QuickFixApplicationResult> = Vec::new();
    let mut diag_counter = 0;

    for file_name in &test_file_names {
        let file_uri = lsconv::file_name_to_document_uri(file_name);

        // Get quick-fix inventory for this file
        let fixes = f.get_quick_fixes_for_diagnostics(t, &file_uri);

        for fix in fixes {
            diag_counter += 1;
            let diag_id = format!("D{diag_counter}");

            inventory.push(QuickFixInventoryEntry {
                id: diag_id.clone(),
                file_uri: file_uri.clone(),
                diagnostic: fix.diagnostic.clone(),
                fix_titles: fix.fix_titles.clone(),
            });

            // Apply each quick fix and collect results
            for (fix_idx, fix_title) in fix.fix_titles.iter().enumerate() {
                if is_disable_style_fix(fix_title) {
                    results.push(QuickFixApplicationResult {
                        diagnostic_id: diag_id.clone(),
                        fix_index: fix_idx,
                        fix_title: fix_title.clone(),
                        skipped: true,
                        ..Default::default()
                    });
                } else {
                    let result = f.apply_quick_fix(t, &file_uri, &fix.diagnostic, fix_idx);
                    results.push(QuickFixApplicationResult {
                        diagnostic_id: diag_id.clone(),
                        fix_index: result.fix_index,
                        fix_title: result.fix_title,
                        changes: result.changes,
                        skipped: false,
                    });
                }
            }
        }
    }

    // Generate baseline
    let base_file_name = tspath::get_base_file_name(&test_file.to_string_lossy());
    let baseline_name = base_file_name.strip_suffix(".ts").unwrap_or(&base_file_name).to_string();
    let baseline_subfolder = version.as_str();
    do_quick_fix_baseline(t, &baseline_name, baseline_subfolder, &inventory, &results);

    f.done(t);
}
