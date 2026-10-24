use std::fmt::Write;

use ets_fourslash::FileChange;
use ets_go::strconv::quote;
use ets_go::testing::T;

use crate::*;

// go test: internal/effecttest/refactor_baseline.go:13 RefactorInventoryEntry
// RefactorInventoryEntry represents one selection range with its available refactor actions.
#[derive(Clone, Debug)]
pub struct RefactorInventoryEntry {
    // ID is the stable range identifier (e.g., "R1", "R2").
    pub id: String,
    // RangeText is a short excerpt of the selected text.
    pub range_text: String,
    // ActionTitles is the list of available refactor action titles.
    pub action_titles: Vec<String>,
    // ActionKinds is the list of code action kinds parallel to ActionTitles.
    pub action_kinds: Vec<String>,
}

// go test: internal/effecttest/refactor_baseline.go:25 RefactorApplicationResult
// RefactorApplicationResult represents the result of applying one refactor action.
#[derive(Clone, Debug)]
pub struct RefactorApplicationResult {
    // RangeID is the stable range identifier (e.g., "R1").
    pub range_id: String,
    // ActionIndex is the index of the refactor action that was applied.
    pub action_index: usize,
    // ActionTitle is the title of the refactor action that was applied.
    pub action_title: String,
    // Changes is the list of file changes from applying the refactor.
    pub changes: Vec<FileChange>,
}

// go test: internal/effecttest/refactor_baseline.go:37 generateRefactorBaseline
// generateRefactorBaseline creates the content for a *.refactors.txt baseline file.
pub(crate) fn generate_refactor_baseline(
    inventory: &[RefactorInventoryEntry],
    results: &[RefactorApplicationResult],
) -> String {
    let mut sb = String::new();

    // === Inventory Section ===
    sb.push_str("=== Refactor Inventory ===\n");

    if inventory.is_empty() {
        sb.push_str("(no selections)\n");
    } else {
        for entry in inventory {
            let _ = write!(sb, "\n[{}] selection: {}\n", entry.id, quote(&entry.range_text));
            if entry.action_titles.is_empty() {
                sb.push_str("  (no refactors)\n");
            } else {
                for (i, title) in entry.action_titles.iter().enumerate() {
                    let _ = writeln!(
                        sb,
                        "  Refactor {i}: {} ({})",
                        quote(title),
                        entry.action_kinds[i]
                    );
                }
            }
        }
    }

    // === Application Results Section ===
    sb.push_str("\n=== Refactor Application Results ===\n");

    if results.is_empty() {
        sb.push_str("(no refactors to apply)\n");
    } else {
        for result in results {
            let _ = write!(
                sb,
                "\n=== [{}] Refactor {}: {} ===\n",
                result.range_id,
                result.action_index,
                quote(&result.action_title),
            );

            if result.changes.is_empty() {
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

// go test: internal/effecttest/refactor_baseline.go:113 DoRefactorBaseline
// DoRefactorBaseline generates a .refactors.txt baseline for Effect refactor tests.
pub fn do_refactor_baseline(
    t: &T,
    baseline_name: &str,
    subfolder: &str,
    inventory: &[RefactorInventoryEntry],
    results: &[RefactorApplicationResult],
) {
    let content = generate_refactor_baseline(inventory, results);
    run_effect_baseline(t, &(baseline_name.to_string() + ".refactors.txt"), &content, subfolder);
}
