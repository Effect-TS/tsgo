use crate::*;

// go test: internal/completions/effect_self_in_classes_test.go:7 TestEffectSelfInClasses_NotInExtendsClause a329ab326e4dbf26
#[test]
fn test_effect_self_in_classes_not_in_extends_clause() {
    go_test(|t| {
        t.parallel();
        // Cursor in a variable declaration, not a class extends clause
        let source = r#"import * as Effect from "effect/Effect"
const x = Effect.Service"#;
        let items = effect_self_in_classes_items(t, source, source.len());
        if !items.is_empty() {
            t.error(format!(
                "expected nil for cursor not in extends clause, got {} items",
                items.len()
            ));
        }
    });
}

// go test: internal/completions/effect_self_in_classes_test.go:18 TestEffectSelfInClasses_NotInClass d2a0c29fce425f15
#[test]
fn test_effect_self_in_classes_not_in_class() {
    go_test(|t| {
        t.parallel();
        // Cursor after a standalone identifier that is not in any class
        let source = r#"import * as Effect from "effect/Effect"
const x = Effect."#;
        let items = effect_self_in_classes_items(t, source, source.len());
        if !items.is_empty() {
            t.error(format!("expected nil for cursor not in class, got {} items", items.len()));
        }
    });
}

// go test: internal/completions/effect_self_in_classes_test.go:29 TestEffectSelfInClasses_EmptySource 7e2ab743fc1bf182
#[test]
fn test_effect_self_in_classes_empty_source() {
    go_test(|t| {
        t.parallel();
        let source = "";
        let items = effect_self_in_classes_items(t, source, 0);
        if !items.is_empty() {
            t.error(format!("expected nil for empty source, got {} items", items.len()));
        }
    });
}

// go test: internal/completions/effect_self_in_classes_test.go:38 TestEffectSelfInClasses_InsideImportDeclaration ee24db6d06858a49
#[test]
fn test_effect_self_in_classes_inside_import_declaration() {
    go_test(|t| {
        t.parallel();
        // Cursor inside an import declaration should not trigger completion
        let source = r#"import { Effect } from "effect""#;
        let pos = "import { Effect".len();
        let items = effect_self_in_classes_items(t, source, pos);
        if !items.is_empty() {
            t.error(format!(
                "expected nil for cursor inside import declaration, got {} items",
                items.len()
            ));
        }
    });
}

// go test: internal/completions/effect_self_in_classes_test.go:49 TestEffectSelfInClasses_InterfaceNotClass c4aff7c94320dd19
#[test]
fn test_effect_self_in_classes_interface_not_class() {
    go_test(|t| {
        t.parallel();
        // Interface extends clause, not a class
        let source = "interface Foo extends Bar";
        let items = effect_self_in_classes_items(t, source, source.len());
        if !items.is_empty() {
            t.error(format!(
                "expected nil for interface extends (not class), got {} items",
                items.len()
            ));
        }
    });
}

// go test: internal/completions/effect_self_in_classes_test.go:59 TestEffectSelfInClasses_AnonymousClass d8f3d7c81b88a519
#[test]
fn test_effect_self_in_classes_anonymous_class() {
    go_test(|t| {
        t.parallel();
        // Anonymous class has no name — should return nil
        let source = "const x = class extends Effect.Service";
        let items = effect_self_in_classes_items(t, source, source.len());
        if !items.is_empty() {
            t.error(format!(
                "expected nil for anonymous class (no name), got {} items",
                items.len()
            ));
        }
    });
}
