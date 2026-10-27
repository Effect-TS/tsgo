use crate::*;

// go test: internal/completions/effect_sql_model_self_in_classes_test.go:7 TestEffectSqlModelSelfInClasses_NotInExtendsClause 821cd68b11bcc458
#[test]
fn test_effect_sql_model_self_in_classes_not_in_extends_clause() {
    go_test(|t| {
        t.parallel();
        // Cursor in a variable declaration, not a class extends clause
        let source = r#"import * as Model from "@effect/sql/Model"
const x = Model.Class"#;
        let items = effect_sql_model_self_in_classes_items(t, source, source.len());
        if !items.is_empty() {
            t.error(format!(
                "expected nil for cursor not in extends clause, got {} items",
                items.len()
            ));
        }
    });
}

// go test: internal/completions/effect_sql_model_self_in_classes_test.go:18 TestEffectSqlModelSelfInClasses_NotInClass c06272f66fd09f66
#[test]
fn test_effect_sql_model_self_in_classes_not_in_class() {
    go_test(|t| {
        t.parallel();
        // Cursor after a standalone identifier that is not in any class
        let source = r#"import * as Model from "@effect/sql/Model"
const x = Model."#;
        let items = effect_sql_model_self_in_classes_items(t, source, source.len());
        if !items.is_empty() {
            t.error(format!("expected nil for cursor not in class, got {} items", items.len()));
        }
    });
}

// go test: internal/completions/effect_sql_model_self_in_classes_test.go:29 TestEffectSqlModelSelfInClasses_EmptySource 3dab9f199f57c2f3
#[test]
fn test_effect_sql_model_self_in_classes_empty_source() {
    go_test(|t| {
        t.parallel();
        let source = "";
        let items = effect_sql_model_self_in_classes_items(t, source, 0);
        if !items.is_empty() {
            t.error(format!("expected nil for empty source, got {} items", items.len()));
        }
    });
}

// go test: internal/completions/effect_sql_model_self_in_classes_test.go:38 TestEffectSqlModelSelfInClasses_InsideImportDeclaration 2c349849161900b8
#[test]
fn test_effect_sql_model_self_in_classes_inside_import_declaration() {
    go_test(|t| {
        t.parallel();
        // Cursor inside an import declaration should not trigger completion
        let source = r#"import { Model } from "@effect/sql""#;
        let pos = "import { Model".len();
        let items = effect_sql_model_self_in_classes_items(t, source, pos);
        if !items.is_empty() {
            t.error(format!(
                "expected nil for cursor inside import declaration, got {} items",
                items.len()
            ));
        }
    });
}

// go test: internal/completions/effect_sql_model_self_in_classes_test.go:49 TestEffectSqlModelSelfInClasses_InterfaceNotClass 5fee33ff588fb25d
#[test]
fn test_effect_sql_model_self_in_classes_interface_not_class() {
    go_test(|t| {
        t.parallel();
        // Interface extends clause, not a class
        let source = "interface Foo extends Bar";
        let items = effect_sql_model_self_in_classes_items(t, source, source.len());
        if !items.is_empty() {
            t.error(format!(
                "expected nil for interface extends (not class), got {} items",
                items.len()
            ));
        }
    });
}

// go test: internal/completions/effect_sql_model_self_in_classes_test.go:59 TestEffectSqlModelSelfInClasses_AnonymousClass 95dced67f8eb9de8
#[test]
fn test_effect_sql_model_self_in_classes_anonymous_class() {
    go_test(|t| {
        t.parallel();
        // Anonymous class has no name — should return nil
        let source = "const x = class extends Model.Class";
        let items = effect_sql_model_self_in_classes_items(t, source, source.len());
        if !items.is_empty() {
            t.error(format!(
                "expected nil for anonymous class (no name), got {} items",
                items.len()
            ));
        }
    });
}
