use crate::*;

// go test: internal/completions/effect_rpc_make_classes_test.go:7 TestRpcMakeClasses_NotInExtendsClause 06ecb53f41dd7c14
#[test]
fn test_rpc_make_classes_not_in_extends_clause() {
    go_test(|t| {
        t.parallel();
        // Cursor in a variable declaration, not a class extends clause
        let source = r#"import * as Rpc from "@effect/rpc"
const x = Rpc.make"#;
        let items = rpc_make_classes_items(t, source, source.len());
        if !items.is_empty() {
            t.error(format!(
                "expected nil for cursor not in extends clause, got {} items",
                items.len()
            ));
        }
    });
}

// go test: internal/completions/effect_rpc_make_classes_test.go:18 TestRpcMakeClasses_NotInClass ed6613e501657ab0
#[test]
fn test_rpc_make_classes_not_in_class() {
    go_test(|t| {
        t.parallel();
        // Cursor after a standalone identifier that is not in any class
        let source = r#"import * as Rpc from "@effect/rpc"
const x = Rpc."#;
        let items = rpc_make_classes_items(t, source, source.len());
        if !items.is_empty() {
            t.error(format!("expected nil for cursor not in class, got {} items", items.len()));
        }
    });
}

// go test: internal/completions/effect_rpc_make_classes_test.go:29 TestRpcMakeClasses_EmptySource a63e489b02a6a93a
#[test]
fn test_rpc_make_classes_empty_source() {
    go_test(|t| {
        t.parallel();
        let source = "";
        let items = rpc_make_classes_items(t, source, 0);
        if !items.is_empty() {
            t.error(format!("expected nil for empty source, got {} items", items.len()));
        }
    });
}

// go test: internal/completions/effect_rpc_make_classes_test.go:38 TestRpcMakeClasses_InsideImportDeclaration 7123d3a653bba3a2
#[test]
fn test_rpc_make_classes_inside_import_declaration() {
    go_test(|t| {
        t.parallel();
        // Cursor inside an import declaration should not trigger completion
        let source = r#"import { Rpc } from "@effect/rpc""#;
        let pos = "import { Rpc".len();
        let items = rpc_make_classes_items(t, source, pos);
        if !items.is_empty() {
            t.error(format!(
                "expected nil for cursor inside import declaration, got {} items",
                items.len()
            ));
        }
    });
}

// go test: internal/completions/effect_rpc_make_classes_test.go:49 TestRpcMakeClasses_InterfaceNotClass 1750583bae624160
#[test]
fn test_rpc_make_classes_interface_not_class() {
    go_test(|t| {
        t.parallel();
        // Interface extends clause, not a class
        let source = "interface Foo extends Bar";
        let items = rpc_make_classes_items(t, source, source.len());
        if !items.is_empty() {
            t.error(format!(
                "expected nil for interface extends (not class), got {} items",
                items.len()
            ));
        }
    });
}

// go test: internal/completions/effect_rpc_make_classes_test.go:59 TestRpcMakeClasses_AnonymousClass a5343076489a6fe2
#[test]
fn test_rpc_make_classes_anonymous_class() {
    go_test(|t| {
        t.parallel();
        // Anonymous class has no name — should return nil
        let source = "const x = class extends Rpc.make";
        let items = rpc_make_classes_items(t, source, source.len());
        if !items.is_empty() {
            t.error(format!(
                "expected nil for anonymous class (no name), got {} items",
                items.len()
            ));
        }
    });
}
