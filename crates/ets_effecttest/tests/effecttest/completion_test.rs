// Rust: Go's blank imports of `etslshooks` and `etstesthooks`: each test registers them (`ets_effecttest::init_hooks`).

use ets_go::testing::T;
use ets_shim::lsproto;
use libtest_mimic::Trial;

use crate::hover_test::plugin_test_content;
use crate::run_case;

// go test: internal/effecttest/completion_test.go:14 completionItemsAtMarker a52fc537824717e8
fn completion_items_at_marker(t: &T, content: &str) -> Vec<lsproto::CompletionItem> {
    let mut f = ets_fourslash::new_fourslash(t, None, content);

    f.go_to_marker(t, "1");
    let completions = f.get_completions(t, None);
    let Some(completions) = completions else {
        t.fatal("completions is nil");
    };
    let items = completions.items;
    f.done(t);
    items
}

// go test: internal/effecttest/completion_test.go:28 findCompletionItem a6f4a6fdbfd26361
fn find_completion_item<'i>(
    items: &'i [lsproto::CompletionItem],
    label: &str,
) -> Option<&'i lsproto::CompletionItem> {
    items.iter().find(|item| item.label == label)
}

// go test: internal/effecttest/completion_test.go:37 TestEffectCompletionExtendsBarrelNamespace 7f51961eaf4921be
fn test_effect_completion_extends_barrel_namespace(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();

    let items = completion_items_at_marker(
        t,
        &plugin_test_content(
            "",
            r#"import * as X from "effect"
class MyService extends X.Context./*1*/"#,
        ),
    );
    let Some(item) = find_completion_item(&items, "Service<MyService, {}>") else {
        t.fatal("expected 'Service<MyService, {}>' completion");
    };
    let new_text = item
        .text_edit
        .as_ref()
        .and_then(|edit| edit.text_edit.as_ref())
        .map(|edit| edit.new_text.as_str());
    if !new_text.is_some_and(|text| text.starts_with("X.Context.Service<MyService, {")) {
        t.error(format!(
            "expected the completion to reference the module through the barrel namespace, got {:?}",
            item.text_edit
        ));
    }
}

// go test: internal/effecttest/completion_test.go:51 TestEffectCompletionDisabled 71703d9abe39242c
fn test_effect_completion_disabled(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();

    let items = completion_items_at_marker(
        t,
        &plugin_test_content(
            r#",
        "completions": false"#,
            r#"import * as Context from "effect/Context"
class MyService extends Context./*1*/"#,
        ),
    );
    for label in ["Service<MyService, {}>", "Service<MyService>({ make })"] {
        if find_completion_item(&items, label).is_some() {
            t.error(format!("did not expect Effect completion {label:?} when completions=false"));
        }
    }
}

/// Rust-only: the trials of `completion_test.go`, named like Go's tests.
pub(crate) fn tests() -> Vec<Trial> {
    vec![
        run_case(
            "TestEffectCompletionExtendsBarrelNamespace".to_string(),
            test_effect_completion_extends_barrel_namespace,
        ),
        run_case("TestEffectCompletionDisabled".to_string(), test_effect_completion_disabled),
    ]
}
