use crate::*;

// go test: internal/completions/context_self_in_classes_test.go:8 TestContextSelfInClasses_NotInExtendsClause 4efe0a87898043ac
#[test]
fn test_context_self_in_classes_not_in_extends_clause() {
    go_test(|t| {
        t.parallel();
        // Cursor in a variable declaration, not a class extends clause
        let source = r#"import * as Context from "effect/Context"
const x = Context.Tag"#;
        let items = context_self_in_classes_items(t, source, source.len());
        if !items.is_empty() {
            t.error(format!(
                "expected nil for cursor not in extends clause, got {} items",
                items.len()
            ));
        }
    });
}

// go test: internal/completions/context_self_in_classes_test.go:19 TestContextSelfInClasses_NotInClass 18bb36f5a45daf46
#[test]
fn test_context_self_in_classes_not_in_class() {
    go_test(|t| {
        t.parallel();
        // Cursor after a standalone identifier that is not in any class
        let source = r#"import * as Context from "effect/Context"
const x = Context."#;
        let items = context_self_in_classes_items(t, source, source.len());
        if !items.is_empty() {
            t.error(format!("expected nil for cursor not in class, got {} items", items.len()));
        }
    });
}

// go test: internal/completions/context_self_in_classes_test.go:30 TestContextSelfInClasses_EmptySource 332024266850246b
#[test]
fn test_context_self_in_classes_empty_source() {
    go_test(|t| {
        t.parallel();
        let source = "";
        let items = context_self_in_classes_items(t, source, 0);
        if !items.is_empty() {
            t.error(format!("expected nil for empty source, got {} items", items.len()));
        }
    });
}

// go test: internal/completions/context_self_in_classes_test.go:39 TestContextSelfInClasses_InsideImportDeclaration a4397fd6c79527b2
#[test]
fn test_context_self_in_classes_inside_import_declaration() {
    go_test(|t| {
        t.parallel();
        // Cursor inside an import declaration should not trigger completion
        let source = r#"import { Context } from "effect""#;
        let pos = "import { Context".len();
        let items = context_self_in_classes_items(t, source, pos);
        if !items.is_empty() {
            t.error(format!(
                "expected nil for cursor inside import declaration, got {} items",
                items.len()
            ));
        }
    });
}

// go test: internal/completions/context_self_in_classes_test.go:50 TestContextSelfInClasses_InterfaceNotClass f7254609d27ca4eb
#[test]
fn test_context_self_in_classes_interface_not_class() {
    go_test(|t| {
        t.parallel();
        // Interface extends clause, not a class
        let source = "interface Foo extends Bar";
        let items = context_self_in_classes_items(t, source, source.len());
        if !items.is_empty() {
            t.error(format!(
                "expected nil for interface extends (not class), got {} items",
                items.len()
            ));
        }
    });
}

// go test: internal/completions/context_self_in_classes_test.go:60 TestContextSelfInClasses_AnonymousClass 6532099d4580fd1d
#[test]
fn test_context_self_in_classes_anonymous_class() {
    go_test(|t| {
        t.parallel();
        // Anonymous class has no name — should return nil
        let source = "const x = class extends Context.Tag";
        let items = context_self_in_classes_items(t, source, source.len());
        if !items.is_empty() {
            t.error(format!(
                "expected nil for anonymous class (no name), got {} items",
                items.len()
            ));
        }
    });
}

// go test: internal/completions/context_self_in_classes_test.go:70 TestContextSelfInClasses_V4NamespaceImport 20d3d8e5a85478f5
#[test]
fn test_context_self_in_classes_v4_namespace_import() {
    go_test(|t| {
        t.parallel();

        let source = r#"import * as Context from "effect/Context"

export class MyService extends Context."#;
        let items = context_self_in_classes_items_with_package_json(t, source, source.len());
        if items.len() != 2 {
            t.fatal(format!("expected 2 items, got {}", items.len()));
        }

        if items[0].label != "Service<MyService, {}>" {
            t.error(format!(
                "item[0].Label = {:?}, want {:?}",
                items[0].label, "Service<MyService, {}>"
            ));
        }
        let got = new_text(&items[0]);
        if !got.starts_with(r#"Context.Service<MyService, {${0}}>()(""#)
            || !got.ends_with(r#""){}"#)
        {
            t.error(format!("item[0].insertText = {got:?}"));
        }

        if items[1].label != "Service<MyService>({ make })" {
            t.error(format!(
                "item[1].Label = {:?}, want {:?}",
                items[1].label, "Service<MyService>({ make })"
            ));
        }
        let got = new_text(&items[1]);
        if got
            != r#"Context.Service<MyService>()("@effect/harness-effect-v4/test/MyService", { make: ${0} }){}"#
        {
            t.error(format!("item[1].insertText = {got:?}"));
        }
    });
}

// go test: internal/completions/context_self_in_classes_test.go:96 TestContextSelfInClasses_V4DirectImport 6ca9d66ef65d2c24
#[test]
fn test_context_self_in_classes_v4_direct_import() {
    go_test(|t| {
        t.parallel();

        let source = r#"import { Service } from "effect/Context"

export class MyService extends Service"#;
        let items = context_self_in_classes_items_with_package_json(t, source, source.len());
        if items.len() != 2 {
            t.fatal(format!("expected 2 items, got {}", items.len()));
        }

        let got = new_text(&items[0]);
        if !got.starts_with(r#"Service<MyService, {${0}}>()(""#) || !got.ends_with(r#""){}"#) {
            t.error(format!("item[0].insertText = {got:?}"));
        }
        let got = new_text(&items[1]);
        if got
            != r#"Service<MyService>()("@effect/harness-effect-v4/test/MyService", { make: ${0} }){}"#
        {
            t.error(format!("item[1].insertText = {got:?}"));
        }
    });
}

// go test: internal/completions/context_self_in_classes_test.go:115 TestContextSelfInClasses_V4IdentifierKeyPattern 0a79139093793ca4
#[test]
fn test_context_self_in_classes_v4_identifier_key_pattern() {
    go_test(|t| {
        t.parallel();

        let source = r#"// @test-config { "keyPatterns": [ { "pattern": "package-identifier", "target": "service" } ] }
import * as Context from "effect/Context"

export class MyService extends Context."#;
        let items = context_self_in_classes_items_with_package_json(t, source, source.len());
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

// go test: internal/completions/context_self_in_classes_test.go:134 TestContextSelfInClasses_V4MiddleOfIdentifier b0cfe0f927a26317
#[test]
fn test_context_self_in_classes_v4_middle_of_identifier() {
    go_test(|t| {
        t.parallel();

        let source = r#"import { Effect, Stream } from "effect"
import * as Context from "effect/Context"

class Foo extends Context.S

Stream.unwrap(Effect.gen(function*() {
	const a = yield* Foo

	return Stream.succeed(a.count)
}))"#;
        let position = source.find("Context.S").unwrap() + "Context.S".len();
        let items = context_self_in_classes_items_with_package_json(t, source, position);
        if items.len() != 2 {
            t.fatal(format!("expected 2 items, got {}", items.len()));
        }

        if items[0].label != "Service<Foo, {}>" {
            t.error(format!("item[0].Label = {:?}, want {:?}", items[0].label, "Service<Foo, {}>"));
        }
        if items[1].label != "Service<Foo>({ make })" {
            t.error(format!(
                "item[1].Label = {:?}, want {:?}",
                items[1].label, "Service<Foo>({ make })"
            ));
        }
        let got = new_text(&items[0]);
        if !got.starts_with(r#"Context.Service<Foo, {${0}}>()(""#) || !got.ends_with(r#""){}"#) {
            t.error(format!("item[0].insertText = {got:?}"));
        }
    });
}
