use ets_go::testing::T;
use ets_shim::lsproto;

use crate::*;

// go test: internal/completions/effect_self_in_classes_test.go:10 selfInClassesItems b5a741c9643ca499
fn self_in_classes_items(t: &T, source: &str, position: usize) -> Vec<lsproto::CompletionItem> {
    t.helper();
    filter_completion_items(completion_items_at(t, source, position), |item| {
        for prefix in ["Service<", r#"Tag(""#, "Class<", r#"make(""#] {
            if item.label.starts_with(prefix) {
                return true;
            }
        }
        false
    })
}

// go test: internal/completions/effect_self_in_classes_test.go:22 TestSelfInClassesCompletions_OnlyInNamedClassExtends df07c490a6ff6662
#[test]
fn test_self_in_classes_completions_only_in_named_class_extends() {
    go_test(|t| {
        t.parallel();

        struct TestCase {
            name: String,
            source: String,
            position: usize,
            want: bool,
        }
        let mut cases = vec![
            TestCase {
                name: "empty source".to_string(),
                source: String::new(),
                position: 0,
                want: false,
            },
            TestCase {
                name: "interface extends".to_string(),
                source: "interface Foo extends Bar".to_string(),
                position: 0,
                want: false,
            },
        ];
        for (name, directive, namespace_imp, access, named_import, imported_ident) in [
            (
                "effect",
                "// @effect-v3\n",
                r#"import * as Effect from "effect/Effect""#,
                "Effect.Service",
                r#"import { Effect } from "effect""#,
                "Effect",
            ),
            (
                "context",
                "",
                r#"import * as Context from "effect/Context""#,
                "Context.Tag",
                r#"import { Context } from "effect""#,
                "Context",
            ),
            (
                "sqlModel",
                "",
                r#"import * as Model from "@effect/sql/Model""#,
                "Model.Class",
                r#"import { Model } from "@effect/sql""#,
                "Model",
            ),
            (
                "rpc",
                "// @effect-v3\n",
                r#"import * as Rpc from "@effect/rpc/Rpc""#,
                "Rpc.make",
                r#"import { Rpc } from "@effect/rpc""#,
                "Rpc",
            ),
        ] {
            let namespace = &access[..=access.find('.').unwrap()];
            let header = format!("{directive}{namespace_imp}");
            let named_import = format!("{directive}{named_import}");
            let position = named_import.find(imported_ident).unwrap() + imported_ident.len();
            cases.extend([
                TestCase {
                    name: format!("{name}/named class extends"),
                    source: format!("{header}\nclass Foo extends {access}"),
                    position: 0,
                    want: true,
                },
                TestCase {
                    name: format!("{name}/variable initializer"),
                    source: format!("{header}\nconst x = {access}"),
                    position: 0,
                    want: false,
                },
                TestCase {
                    name: format!("{name}/member access outside class"),
                    source: format!("{header}\nconst x = {namespace}"),
                    position: 0,
                    want: false,
                },
                TestCase {
                    name: format!("{name}/anonymous class"),
                    source: format!("{header}\nconst x = class extends {access}"),
                    position: 0,
                    want: false,
                },
                TestCase {
                    name: format!("{name}/inside import declaration"),
                    source: named_import,
                    position,
                    want: false,
                },
            ]);
        }

        for tc in &cases {
            t.run(&tc.name, |t| {
                t.parallel();
                let mut position = tc.position;
                if position == 0 {
                    position = tc.source.len();
                }
                let items = self_in_classes_items(t, &tc.source, position);
                if tc.want && items.is_empty() {
                    t.error(format!("expected self-in-classes completions for {:?}", tc.source));
                }
                if !tc.want && !items.is_empty() {
                    t.error(format!(
                        "expected no self-in-classes completions for {:?}, got {} items",
                        tc.source,
                        items.len()
                    ));
                }
            });
        }
    });
}
