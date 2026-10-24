// Rust: Go's blank imports of `etslshooks` and `etstesthooks`: each test registers them (`ets_effecttest::init_hooks`).

use ets_go::testing::T;
use ets_shim::lsproto;
use libtest_mimic::Trial;

use crate::run_case;

// go test: internal/effecttest/completion_test.go:13 findCompletionLabel
fn find_completion_label(items: &[lsproto::CompletionItem], label: &str) -> bool {
    for item in items {
        if item.label == label {
            return true;
        }
    }
    false
}

// go test: internal/effecttest/completion_test.go:22 TestEffectCompletionExtendsServiceMap
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

// go test: internal/effecttest/completion_test.go:60 TestEffectCompletionExtendsSchema
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

// go test: internal/effecttest/completion_test.go:104 TestEffectCompletionDisabled
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
            "TestEffectCompletionExtendsSchema".to_string(),
            test_effect_completion_extends_schema,
        ),
        run_case("TestEffectCompletionDisabled".to_string(), test_effect_completion_disabled),
    ]
}
