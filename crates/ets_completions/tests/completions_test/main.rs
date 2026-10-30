//! Rust-only: the test binary of the ported `internal/completions` Go tests (`package completions_test`), one module
//! per Go file; this file holds `fourslash_test.go`. Go's blank imports of `etslshooks` and `etstesthooks` (and
//! `etscheckerhooks`, imported by `effect_schema_self_in_classes_test.go`) are the `init_hooks` call at the start of
//! every test.
//!
//! Go's "nil" item lists (`filterCompletionItems` returns nil for an empty result) are empty `Vec`s.

mod context_self_in_classes_test;
mod duration_input_test;
mod effect_codegens_comment_test;
mod effect_diagnostics_comment_test;
mod effect_jsdoc_comment_test;
mod effect_schema_self_in_classes_test;
mod effect_self_in_classes_test;
mod error_key_patterns_test;
mod fn_function_star_test;
mod schema_brand_test;

use ets_go::testing::T;
use ets_shim::lsproto;

/// Rust-only: runs a Go test body (`func TestX(t *testing.T)`) on a thread with the checker's stack, after
/// registering the hooks Go's blank imports register.
pub(crate) fn go_test(body: impl FnOnce(&T) + Send + 'static) {
    ets_go::testing::on_checker_stack(move || {
        init_hooks();
        ets_go::testing::run(body);
    });
}

/// Rust-only: Go's blank imports of `etscheckerhooks`, `etslshooks` and `etstesthooks` (each `init` runs once).
/// A blank import in one file of a Go test package registers the hooks for the whole test binary, as here.
fn init_hooks() {
    ets_etscheckerhooks::init();
    ets_etslshooks::init();
    ets_etstesthooks::init();
}

// go test: internal/completions/fourslash_test.go:15 completionTestTsConfig 9b1d4cf9cd83e9bc
pub(crate) const COMPLETION_TEST_TS_CONFIG: &str = r#"{
  "compilerOptions": {
    "strict": true,
    "target": "ESNext",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "plugins": [
      {
        "name": "@effect/language-service"
      }
    ]
  }
}"#;

// go test: internal/completions/fourslash_test.go:29 harnessPackageJSON 3907809bc130966a
pub(crate) const HARNESS_PACKAGE_JSON: &str = r#"{ "name": "@effect/harness-effect-v4" }"#;

// go test: internal/completions/fourslash_test.go:31 completionItemsAt 85078df334181dba
pub(crate) fn completion_items_at(
    t: &T,
    source: &str,
    position: usize,
) -> Vec<lsproto::CompletionItem> {
    t.helper();
    completion_items_in(t, COMPLETION_TEST_TS_CONFIG, "", source, position)
}

// go test: internal/completions/fourslash_test.go:36 completionItemsIn 974b851ab004090c
pub(crate) fn completion_items_in(
    t: &T,
    tsconfig: &str,
    package_json: &str,
    source: &str,
    position: usize,
) -> Vec<lsproto::CompletionItem> {
    t.helper();

    if position > source.len() {
        t.fatal(format!(
            "invalid completion position {position} for source length {}",
            source.len()
        ));
    }

    let mut content = "// @Filename: /tsconfig.json\n".to_string() + tsconfig + "\n";
    if !package_json.is_empty() {
        content = content + "// @Filename: /package.json\n" + package_json + "\n";
    }
    content =
        content + "// @Filename: /test.ts\n" + &source[..position] + "/*1*/" + &source[position..];

    let mut f = ets_fourslash::new_fourslash(t, None, &content);

    f.go_to_marker(t, "1");
    let completions = f.get_completions(t, None);
    f.done(t);
    let Some(completions) = completions else {
        return Vec::new();
    };
    completions.items
}

// go test: internal/completions/fourslash_test.go:60 filterCompletionItems bafb82cb32041e59
pub(crate) fn filter_completion_items(
    items: Vec<lsproto::CompletionItem>,
    keep: impl Fn(&lsproto::CompletionItem) -> bool,
) -> Vec<lsproto::CompletionItem> {
    items.into_iter().filter(|item| keep(item)).collect()
}

// go test: internal/completions/fourslash_test.go:76 findCompletionItem a6f4a6fdbfd26361
pub(crate) fn find_completion_item<'a>(
    items: &'a [lsproto::CompletionItem],
    label: &str,
) -> Option<&'a lsproto::CompletionItem> {
    items.iter().find(|item| item.label == label)
}

// go test: internal/completions/fourslash_test.go:85 fnFunctionStarItems fb56911749cca170
pub(crate) fn fn_function_star_items(
    t: &T,
    source: &str,
    position: usize,
) -> Vec<lsproto::CompletionItem> {
    t.helper();
    filter_completion_items(completion_items_at(t, source, position), |item| {
        item.label.starts_with(r#"fn(""#)
            || item.label == "fn(function*(){})"
            || item.label == "fnUntraced(function*(){})"
    })
}

// go test: internal/completions/fourslash_test.go:92 labeledItems 41a6cf71f85702ba
pub(crate) fn labeled_items(
    t: &T,
    source: &str,
    position: usize,
    labels: &[&str],
) -> Vec<lsproto::CompletionItem> {
    t.helper();
    filter_completion_items(completion_items_at(t, source, position), |item| {
        labels.contains(&item.label.as_str())
    })
}

// go test: internal/completions/fourslash_test.go:99 runCommentDirectiveTriggerCases 3ac98edbe88c94c9
pub(crate) fn run_comment_directive_trigger_cases(t: &T, labels: &[&str]) {
    t.helper();
    for (name, source, trigger) in [
        ("line comment", "// @", true),
        ("block comment", "/* @", true),
        ("jsdoc comment", "/** @", true),
        ("extra whitespace", "//  @  ", true),
        ("second line", "const x = 1\n// @", true),
        ("no at sign", "// some comment without at", false),
        ("at outside comment", "const x = @", false),
    ] {
        t.run(name, |t| {
            t.parallel();
            let items = labeled_items(t, source, source.len(), labels);
            if !trigger {
                if !items.is_empty() {
                    t.error(format!("expected no items for {source:?}, got {}", items.len()));
                }
                return;
            }
            if items.len() != labels.len() {
                t.fatal(format!(
                    "expected {} items for {source:?}, got {}",
                    labels.len(),
                    items.len()
                ));
            }
            let at = source.rfind('@').unwrap();
            let want_line = source[..at].matches('\n').count();
            let want_character = at - source[..at].rfind('\n').map_or(0, |i| i + 1);
            for (i, item) in items.iter().enumerate() {
                if item.label != labels[i] {
                    t.error(format!("item[{i}].Label = {:?}, want {:?}", item.label, labels[i]));
                }
                let start = edit_range(item).start;
                if start.line as usize != want_line || start.character as usize != want_character {
                    t.error(format!(
                        "item[{i}] replacement starts at {}:{}, want {want_line}:{want_character}",
                        start.line, start.character
                    ));
                }
            }
        });
    }
}

/// Rust-only: Go's `items[i].TextEdit.TextEdit.NewText` (a nil pointer panics in Go as `unwrap` does here).
pub(crate) fn new_text(item: &lsproto::CompletionItem) -> &str {
    &item.text_edit.as_ref().unwrap().text_edit.as_ref().unwrap().new_text
}

/// Rust-only: Go's `items[i].TextEdit.TextEdit.Range`.
pub(crate) fn edit_range(item: &lsproto::CompletionItem) -> lsproto::Range {
    item.text_edit.as_ref().unwrap().text_edit.as_ref().unwrap().range
}
