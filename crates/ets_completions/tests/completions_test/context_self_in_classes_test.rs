use ets_go::testing::T;
use ets_shim::lsproto;

use crate::*;

// go test: internal/completions/context_self_in_classes_test.go:10 contextServiceItems d3899223faaa106d
fn context_service_items(t: &T, source: &str, position: usize) -> Vec<lsproto::CompletionItem> {
    t.helper();
    filter_completion_items(
        completion_items_in(t, COMPLETION_TEST_TS_CONFIG, HARNESS_PACKAGE_JSON, source, position),
        |item| item.label.starts_with(r#"Tag(""#) || item.label.starts_with("Service<"),
    )
}

// go test: internal/completions/context_self_in_classes_test.go:17 TestContextSelfInClasses_V4Service 9cf5a836bdc89657
#[test]
fn test_context_self_in_classes_v4_service() {
    go_test(|t| {
        t.parallel();

        struct Case {
            name: &'static str,
            source: &'static str,
            cursor: &'static str,
            class_name: &'static str,
            access: &'static str,
        }
        for tc in [
            Case {
                name: "namespace import",
                source: r#"import * as Context from "effect/Context"

export class MyService extends Context."#,
                cursor: "",
                class_name: "MyService",
                access: "Context.",
            },
            Case {
                name: "direct import",
                source: r#"import { Service } from "effect/Context"

export class MyService extends Service"#,
                cursor: "",
                class_name: "MyService",
                access: "",
            },
            Case {
                name: "middle of file",
                source: r#"import { Effect, Stream } from "effect"
import * as Context from "effect/Context"

class Foo extends Context.S

Stream.unwrap(Effect.gen(function*() {
	const a = yield* Foo

	return Stream.succeed(a.count)
}))"#,
                cursor: "Context.S",
                class_name: "Foo",
                access: "Context.",
            },
        ] {
            t.run(tc.name, |t| {
                t.parallel();
                let mut position = tc.source.len();
                if !tc.cursor.is_empty() {
                    position = tc.source.find(tc.cursor).unwrap() + tc.cursor.len();
                }
                let items = context_service_items(t, tc.source, position);
                if items.len() != 2 {
                    t.fatal(format!("expected 2 items, got {}", items.len()));
                }

                let want = format!("Service<{}, {{}}>", tc.class_name);
                if items[0].label != want {
                    t.error(format!("item[0].Label = {:?}, want {want:?}", items[0].label));
                }
                let got = new_text(&items[0]);
                if !got.starts_with(&format!(
                    "{}Service<{}, {{${{0}}}}>()(\"",
                    tc.access, tc.class_name
                )) || !got.ends_with(r#""){}"#)
                {
                    t.error(format!("item[0].insertText = {got:?}"));
                }

                let want = format!("Service<{}>({{ make }})", tc.class_name);
                if items[1].label != want {
                    t.error(format!("item[1].Label = {:?}, want {want:?}", items[1].label));
                }
                let got = new_text(&items[1]);
                let want = format!(
                    "{}Service<{}>()(\"@effect/harness-effect-v4/test/{}\", {{ make: ${{0}} }}){{}}",
                    tc.access, tc.class_name, tc.class_name
                );
                if got != want {
                    t.error(format!("item[1].insertText = {got:?}, want {want:?}"));
                }
            });
        }
    });
}

// go test: internal/completions/context_self_in_classes_test.go:87 TestContextSelfInClasses_V4IdentifierKeyPattern 38369aa698d03096
#[test]
fn test_context_self_in_classes_v4_identifier_key_pattern() {
    go_test(|t| {
        t.parallel();

        let source = r#"// @test-config { "keyPatterns": [ { "pattern": "package-identifier", "target": "service" } ] }
import * as Context from "effect/Context"

export class MyService extends Context."#;
        let items = context_service_items(t, source, source.len());
        if items.len() != 2 {
            t.fatal(format!("expected 2 items, got {}", items.len()));
        }

        for (i, item) in items.iter().enumerate() {
            if !new_text(item).contains(r#""@effect/harness-effect-v4/test/MyService""#) {
                t.error(format!(
                    "item[{i}].insertText = {:?}, want package identifier key",
                    new_text(item)
                ));
            }
        }
    });
}
