use std::fmt::Write;

use ets_fourslash::FileChange;
use ets_go::strconv::quote;
use ets_go::testing::T;
use ets_shim::lsproto;

use crate::*;

// go test: internal/effecttest/quickfix_baseline.go:15 QuickFixInventoryEntry f7d9161bb0503989
// QuickFixInventoryEntry represents one diagnostic occurrence with its available quick fixes.
#[derive(Clone, Debug)]
pub struct QuickFixInventoryEntry {
    // ID is the stable diagnostic identifier (e.g., "D1", "D2").
    pub id: String,
    // FileURI is the document URI of the file containing the diagnostic.
    pub file_uri: lsproto::DocumentUri,
    // Diagnostic is the LSP diagnostic.
    pub diagnostic: lsproto::Diagnostic,
    // FixTitles is the list of available quick-fix titles.
    pub fix_titles: Vec<String>,
}

// go test: internal/effecttest/quickfix_baseline.go:27 QuickFixApplicationResult bebfdde4374f4a9c
// QuickFixApplicationResult represents the result of applying one quick fix.
#[derive(Clone, Debug, Default)]
pub struct QuickFixApplicationResult {
    // DiagnosticID is the stable diagnostic identifier (e.g., "D1").
    pub diagnostic_id: String,
    // FixIndex is the index of the quick fix that was applied.
    pub fix_index: usize,
    // FixTitle is the title of the quick fix that was applied.
    pub fix_title: String,
    // Changes is the list of file changes from applying the quick fix.
    pub changes: Vec<FileChange>,
    // Skipped indicates the fix was intentionally not applied (e.g., disable-style fixes).
    pub skipped: bool,
}

// go test: internal/effecttest/quickfix_baseline.go:41 diagnosticCodeString 94e3f664cee6b5d5
// diagnosticCodeString formats the diagnostic code for display.
pub(crate) fn diagnostic_code_string(d: &lsproto::Diagnostic) -> String {
    let Some(code) = &d.code else {
        return "?".to_string();
    };
    if let Some(integer) = code.integer {
        return integer.to_string();
    }
    if let Some(string) = &code.string {
        return string.clone();
    }
    "?".to_string()
}

// go test: internal/effecttest/quickfix_baseline.go:54 diagnosticMessageString 6224ffe2dc317dc2
pub(crate) fn diagnostic_message_string(d: &lsproto::Diagnostic) -> String {
    if let Some(string) = &d.message.string {
        return string.clone();
    }
    if let Some(markup_content) = &d.message.markup_content {
        return markup_content.value.clone();
    }
    String::new()
}

// go test: internal/effecttest/quickfix_baseline.go:65 generateQuickFixBaseline c72a4cf0458f5456
// generateQuickFixBaseline creates the content for a *.quickfixes.txt baseline file.
pub(crate) fn generate_quick_fix_baseline(
    inventory: &[QuickFixInventoryEntry],
    results: &[QuickFixApplicationResult],
) -> String {
    let mut sb = String::new();

    // === Inventory Section ===
    sb.push_str("=== Quick Fix Inventory ===\n");

    if inventory.is_empty() {
        sb.push_str("(no diagnostics)\n");
    } else {
        for entry in inventory {
            let _ = write!(
                sb,
                "\n[{}] ({}:{}-{}:{}) TS{}: {}\n",
                entry.id,
                entry.diagnostic.range.start.line + 1,
                entry.diagnostic.range.start.character + 1,
                entry.diagnostic.range.end.line + 1,
                entry.diagnostic.range.end.character + 1,
                diagnostic_code_string(&entry.diagnostic),
                diagnostic_message_string(&entry.diagnostic),
            );
            if entry.fix_titles.is_empty() {
                sb.push_str("  (no quick fixes)\n");
            } else {
                for (i, title) in entry.fix_titles.iter().enumerate() {
                    let _ = writeln!(sb, "  Fix {i}: {}", quote(title));
                }
            }
        }
    }

    // === Application Results Section ===
    sb.push_str("\n=== Quick Fix Application Results ===\n");

    if results.is_empty() {
        sb.push_str("(no quick fixes to apply)\n");
    } else {
        for result in results {
            let _ = write!(
                sb,
                "\n=== [{}] Fix {}: {} ===\n",
                result.diagnostic_id,
                result.fix_index,
                quote(&result.fix_title),
            );

            if result.skipped {
                sb.push_str("skipped by default\n");
            } else if result.changes.is_empty() {
                sb.push_str("(no changes)\n");
            } else {
                // Sort changes by URI for deterministic output
                let mut sorted_changes = result.changes.clone();
                ets_go::sort::slice(&mut sorted_changes, |a, b| a.uri.0 < b.uri.0);

                for change in &sorted_changes {
                    let _ = write!(sb, "\n--- {} ---\n", change.uri.0);

                    if change.created {
                        sb.push_str("(file created)\n");
                        sb.push_str(&change.after);
                        if !change.after.ends_with('\n') {
                            sb.push('\n');
                        }
                    } else if change.deleted {
                        sb.push_str("(file deleted)\n");
                        sb.push_str("Before:\n");
                        sb.push_str(&change.before);
                        if !change.before.ends_with('\n') {
                            sb.push('\n');
                        }
                    } else {
                        sb.push_str(&change.after);
                        if !change.after.ends_with('\n') {
                            sb.push('\n');
                        }
                    }
                }
            }
        }
    }

    sb
}

// go test: internal/effecttest/quickfix_baseline.go:152 DoQuickFixBaseline b277f76292cb6483
// DoQuickFixBaseline generates a .quickfixes.txt baseline for Effect tests.
pub fn do_quick_fix_baseline(
    t: &T,
    baseline_name: &str,
    subfolder: &str,
    inventory: &[QuickFixInventoryEntry],
    results: &[QuickFixApplicationResult],
) {
    let content = generate_quick_fix_baseline(inventory, results);
    run_effect_baseline(t, &(baseline_name.to_string() + ".quickfixes.txt"), &content, subfolder);
}
