// Rust: Go's blank imports of `etslshooks` and `etstesthooks`: each test registers them (`ets_effecttest::init_hooks`).

use ets_go::testing::T;
use ets_shim::lsproto;
use libtest_mimic::Trial;

use crate::run_case;

// go test: internal/effecttest/completion_test.go:14 findCompletionLabel
fn find_completion_label(items: &[lsproto::CompletionItem], label: &str) -> bool {
    find_completion_item(items, label).is_some()
}

// go test: internal/effecttest/completion_test.go:18 findCompletionItem
fn find_completion_item<'i>(
    items: &'i [lsproto::CompletionItem],
    label: &str,
) -> Option<&'i lsproto::CompletionItem> {
    items.iter().find(|item| item.label == label)
}

// go test: internal/effecttest/completion_test.go:27 TestEffectCompletionExtendsServiceMap
fn test_effect_completion_extends_service_map(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();

    let content = r#"// @Filename: /tsconfig.json
{
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
}
// @Filename: /test.ts
import * as Context from "effect/Context"
class MyService extends Context./*1*/"#;

    let mut f = ets_fourslash::new_fourslash(t, None /*capabilities*/, content);

    f.go_to_marker(t, "1");
    let completions = f.get_completions(t, None);
    match completions {
        None => t.fatal("completions is nil"),
        Some(completions) => {
            if !find_completion_label(&completions.items, "Service<MyService, {}>") {
                t.error("expected 'Service<MyService, {}>' completion");
            }
            if !find_completion_label(&completions.items, "Service<MyService>({ make })") {
                t.error("expected 'Service<MyService>({ make })' completion");
            }
        }
    }
    f.done(t);
}

// go test: internal/effecttest/completion_test.go:65 TestEffectCompletionExtendsBarrelNamespace
fn test_effect_completion_extends_barrel_namespace(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();

    let content = r#"// @Filename: /tsconfig.json
{
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
}
// @Filename: /test.ts
import * as X from "effect"
class MyService extends X.Context./*1*/"#;

    let mut f = ets_fourslash::new_fourslash(t, None /*capabilities*/, content);

    f.go_to_marker(t, "1");
    let completions = f.get_completions(t, None);
    let Some(completions) = completions else {
        t.fatal("completions is nil");
    };
    let Some(item) = find_completion_item(&completions.items, "Service<MyService, {}>") else {
        t.fatal("expected 'Service<MyService, {}>' completion");
    };
    let new_text = item
        .text_edit
        .as_ref()
        .and_then(|edit| edit.text_edit.as_ref())
        .map(|edit| edit.new_text.as_str());
    if !new_text.is_some_and(|text| text.starts_with("X.Context.Service<MyService, {")) {
        t.error(format!(
            "expected the completion to reference the module through the barrel namespace, got {new_text:?}"
        ));
    }
    f.done(t);
}

// go test: internal/effecttest/completion_test.go:103 TestEffectCompletionExtendsSchema
fn test_effect_completion_extends_schema(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();

    let content = r#"// @Filename: /tsconfig.json
{
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
}
// @Filename: /test.ts
import { Schema } from "effect"
class Foo extends Schema./*1*/"#;

    let mut f = ets_fourslash::new_fourslash(t, None /*capabilities*/, content);

    f.go_to_marker(t, "1");
    let completions = f.get_completions(t, None);
    match completions {
        None => t.fatal("completions is nil"),
        Some(completions) => {
            if !find_completion_label(&completions.items, "Class<Foo>") {
                t.error("expected 'Class<Foo>' completion");
            }
            if !find_completion_label(&completions.items, "TaggedClass<Foo>") {
                t.error("expected 'TaggedClass<Foo>' completion");
            }
            if !find_completion_label(&completions.items, "TaggedError<Foo>") {
                t.error("expected 'TaggedError<Foo>' completion");
            }
            if !find_completion_label(&completions.items, "Error<Foo>") {
                t.error("expected 'Error<Foo>' completion");
            }
        }
    }
    f.done(t);
}

// go test: internal/effecttest/completion_test.go:147 TestEffectCompletionDisabled
fn test_effect_completion_disabled(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();

    let content = r#"// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "strict": true,
    "target": "ESNext",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "plugins": [
      {
        "name": "@effect/language-service",
        "completions": false
      }
    ]
  }
}
// @Filename: /test.ts
import * as Context from "effect/Context"
class MyService extends Context./*1*/"#;

    let mut f = ets_fourslash::new_fourslash(t, None /*capabilities*/, content);

    f.go_to_marker(t, "1");
    let Some(completions) = f.get_completions(t, None) else {
        t.fatal("completions is nil");
    };
    if find_completion_label(&completions.items, "Service<MyService, {}>") {
        t.error("did not expect Effect completion when completions=false");
    }
    if find_completion_label(&completions.items, "Service<MyService>({ make })") {
        t.error("did not expect Effect completion when completions=false");
    }
    f.done(t);
}

/// Rust-only: the trials of `completion_test.go`, named like Go's tests.
pub(crate) fn tests() -> Vec<Trial> {
    vec![
        run_case(
            "TestEffectCompletionExtendsServiceMap".to_string(),
            test_effect_completion_extends_service_map,
        ),
        run_case(
            "TestEffectCompletionExtendsBarrelNamespace".to_string(),
            test_effect_completion_extends_barrel_namespace,
        ),
        run_case(
            "TestEffectCompletionExtendsSchema".to_string(),
            test_effect_completion_extends_schema,
        ),
        run_case("TestEffectCompletionDisabled".to_string(), test_effect_completion_disabled),
    ]
}
