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
mod effect_rpc_make_classes_test;
mod effect_schema_self_in_classes_test;
mod effect_self_in_classes_test;
mod effect_sql_model_self_in_classes_test;
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

// go test: internal/completions/fourslash_test.go:29 completionItemsAt 0efe0a729348a592
pub(crate) fn completion_items_at(
    t: &T,
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

    let source_with_marker = format!("{}/*1*/{}", &source[..position], &source[position..]);
    let content = format!(
        "// @Filename: /tsconfig.json\n{COMPLETION_TEST_TS_CONFIG}\n// @Filename: /test.ts\n{source_with_marker}"
    );

    let mut f = ets_fourslash::new_fourslash(t, None, &content);

    f.go_to_marker(t, "1");
    let completions = f.get_completions(t, None);
    f.done(t);
    let Some(completions) = completions else {
        return Vec::new();
    };
    completions.items
}

// go test: internal/completions/fourslash_test.go:51 completionItemsAtWithPackageJSON 106832f810f8a102
pub(crate) fn completion_items_at_with_package_json(
    t: &T,
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

    let source_with_marker = format!("{}/*1*/{}", &source[..position], &source[position..]);
    let content = format!(
        "// @Filename: /tsconfig.json\n{COMPLETION_TEST_TS_CONFIG}\n// @Filename: /package.json\n{package_json}\n// @Filename: /test.ts\n{source_with_marker}"
    );

    let mut f = ets_fourslash::new_fourslash(t, None, &content);

    f.go_to_marker(t, "1");
    let completions = f.get_completions(t, None);
    f.done(t);
    let Some(completions) = completions else {
        return Vec::new();
    };
    completions.items
}

// go test: internal/completions/fourslash_test.go:74 filterCompletionItems bafb82cb32041e59
pub(crate) fn filter_completion_items(
    items: Vec<lsproto::CompletionItem>,
    keep: impl Fn(&lsproto::CompletionItem) -> bool,
) -> Vec<lsproto::CompletionItem> {
    items.into_iter().filter(|item| keep(item)).collect()
}

// go test: internal/completions/fourslash_test.go:90 findCompletionItem a6f4a6fdbfd26361
pub(crate) fn find_completion_item<'a>(
    items: &'a [lsproto::CompletionItem],
    label: &str,
) -> Option<&'a lsproto::CompletionItem> {
    items.iter().find(|item| item.label == label)
}

// go test: internal/completions/fourslash_test.go:99 fnFunctionStarItems fb56911749cca170
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

// go test: internal/completions/fourslash_test.go:106 effectJsdocCommentItems 04e61bcc80bfe785
pub(crate) fn effect_jsdoc_comment_items(
    t: &T,
    source: &str,
    position: usize,
) -> Vec<lsproto::CompletionItem> {
    t.helper();
    filter_completion_items(completion_items_at(t, source, position), |item| {
        item.label == "@effect-identifier"
    })
}

// go test: internal/completions/fourslash_test.go:113 effectCodegensCommentItems 3d688c18669bc6fe
pub(crate) fn effect_codegens_comment_items(
    t: &T,
    source: &str,
    position: usize,
) -> Vec<lsproto::CompletionItem> {
    t.helper();
    filter_completion_items(completion_items_at(t, source, position), |item| {
        item.label == "@effect-codegens"
    })
}

// go test: internal/completions/fourslash_test.go:120 effectDiagnosticsCommentItems 45f107a8ab889340
pub(crate) fn effect_diagnostics_comment_items(
    t: &T,
    source: &str,
    position: usize,
) -> Vec<lsproto::CompletionItem> {
    t.helper();
    filter_completion_items(completion_items_at(t, source, position), |item| {
        item.label == "@effect-diagnostics" || item.label == "@effect-diagnostics-next-line"
    })
}

// go test: internal/completions/fourslash_test.go:127 durationInputItems faaa0afc25a5cc71
pub(crate) fn duration_input_items(
    t: &T,
    source: &str,
    position: usize,
) -> Vec<lsproto::CompletionItem> {
    t.helper();
    filter_completion_items(completion_items_at(t, source, position), |item| {
        ["nanos", "micros", "millis", "seconds", "minutes", "hours", "days", "weeks"]
            .contains(&item.label.as_str())
    })
}

// go test: internal/completions/fourslash_test.go:134 effectSelfInClassesItems 7401f14c674bb805
pub(crate) fn effect_self_in_classes_items(
    t: &T,
    source: &str,
    position: usize,
) -> Vec<lsproto::CompletionItem> {
    t.helper();
    filter_completion_items(completion_items_at(t, source, position), |item| {
        item.label.starts_with("Service<") || item.label.starts_with(r#"Tag(""#)
    })
}

// go test: internal/completions/fourslash_test.go:141 contextSelfInClassesItems daba8b5542e70518
pub(crate) fn context_self_in_classes_items(
    t: &T,
    source: &str,
    position: usize,
) -> Vec<lsproto::CompletionItem> {
    t.helper();
    filter_completion_items(completion_items_at(t, source, position), |item| {
        item.label.starts_with(r#"Tag(""#)
    })
}

// go test: internal/completions/fourslash_test.go:148 contextSelfInClassesItemsWithPackageJSON 3d9a8c0f60cd9adb
pub(crate) fn context_self_in_classes_items_with_package_json(
    t: &T,
    source: &str,
    position: usize,
) -> Vec<lsproto::CompletionItem> {
    t.helper();
    const PACKAGE_JSON: &str = r#"{
	  "name": "@effect/harness-effect-v4"
	}"#;
    filter_completion_items(
        completion_items_at_with_package_json(t, PACKAGE_JSON, source, position),
        |item| item.label.starts_with(r#"Tag(""#) || item.label.starts_with("Service<"),
    )
}

// go test: internal/completions/fourslash_test.go:158 effectSQLModelSelfInClassesItems 011e7bd874e5d46c
pub(crate) fn effect_sql_model_self_in_classes_items(
    t: &T,
    source: &str,
    position: usize,
) -> Vec<lsproto::CompletionItem> {
    t.helper();
    filter_completion_items(completion_items_at(t, source, position), |item| {
        item.label.starts_with("Class<")
    })
}

// go test: internal/completions/fourslash_test.go:165 rpcMakeClassesItems 22c0dc870abd18bb
pub(crate) fn rpc_make_classes_items(
    t: &T,
    source: &str,
    position: usize,
) -> Vec<lsproto::CompletionItem> {
    t.helper();
    filter_completion_items(completion_items_at(t, source, position), |item| {
        item.label.starts_with(r#"make(""#)
    })
}

// go test: internal/completions/fourslash_test.go:172 schemaBrandItems db5c66bf239f9abd
pub(crate) fn schema_brand_items(
    t: &T,
    source: &str,
    position: usize,
) -> Vec<lsproto::CompletionItem> {
    t.helper();
    filter_completion_items(completion_items_at(t, source, position), |item| {
        item.label.starts_with(r#"brand(""#)
    })
}

/// Rust-only: Go's `items[i].TextEdit.TextEdit.NewText` (a nil pointer panics in Go as `unwrap` does here).
pub(crate) fn new_text(item: &lsproto::CompletionItem) -> &str {
    &item.text_edit.as_ref().unwrap().text_edit.as_ref().unwrap().new_text
}

/// Rust-only: Go's `items[i].TextEdit.TextEdit.Range`.
pub(crate) fn edit_range(item: &lsproto::CompletionItem) -> lsproto::Range {
    item.text_edit.as_ref().unwrap().text_edit.as_ref().unwrap().range
}
