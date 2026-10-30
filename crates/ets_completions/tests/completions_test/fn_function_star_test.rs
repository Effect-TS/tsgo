use crate::*;

// go test: internal/completions/fn_function_star_test.go:7 TestFnFunctionStar_NotEffectModule e779d69965d9803e
#[test]
fn test_fn_function_star_not_effect_module() {
    go_test(|t| {
        t.parallel();
        let source = r#"import * as Foo from "some-lib"
const x = Foo.fn"#;
        let items = fn_function_star_items(t, source, source.len());
        if !items.is_empty() {
            t.error(format!(
                "expected nil for non-Effect module access, got {} items",
                items.len()
            ));
        }
    });
}

// go test: internal/completions/fn_function_star_test.go:17 TestFnFunctionStar_NamespaceImport_InVarDecl d261b688cc73a2bf
#[test]
fn test_fn_function_star_namespace_import_in_var_decl() {
    go_test(|t| {
        t.parallel();
        let source = r#"import * as Effect from "effect/Effect"
const myFn = Effect.fn"#;
        let items = fn_function_star_items(t, source, source.len());
        if items.len() != 3 {
            t.fatal(format!("expected 3 items, got {}", items.len()));
        }
        if items[0].label != r#"fn("myFn")"# {
            t.error(format!("item[0].Label = {:?}, want {:?}", items[0].label, r#"fn("myFn")"#));
        }
        if items[1].label != "fn(function*(){})" {
            t.error(format!(
                "item[1].Label = {:?}, want {:?}",
                items[1].label, "fn(function*(){})"
            ));
        }
        if items[2].label != "fnUntraced(function*(){})" {
            t.error(format!(
                "item[2].Label = {:?}, want {:?}",
                items[2].label, "fnUntraced(function*(){})"
            ));
        }
    });
}

// go test: internal/completions/fn_function_star_test.go:36 TestFnFunctionStar_NamedImport_InVarDecl 7256f55d304e2284
#[test]
fn test_fn_function_star_named_import_in_var_decl() {
    go_test(|t| {
        t.parallel();
        let source = r#"import { Effect } from "effect"
const myFn = Effect.fn"#;
        let items = fn_function_star_items(t, source, source.len());
        if items.len() != 3 {
            t.fatal(format!("expected 3 items, got {}", items.len()));
        }
        if items[0].label != r#"fn("myFn")"# {
            t.error(format!("item[0].Label = {:?}, want {:?}", items[0].label, r#"fn("myFn")"#));
        }
    });
}

// go test: internal/completions/fn_function_star_test.go:49 TestFnFunctionStar_NotInVarDecl dc75f40af493ec6b
#[test]
fn test_fn_function_star_not_in_var_decl() {
    go_test(|t| {
        t.parallel();
        let source = r#"import * as Effect from "effect/Effect"
Effect.fn"#;
        let items = fn_function_star_items(t, source, source.len());
        if items.len() != 2 {
            t.fatal(format!("expected 2 items (no named variant), got {}", items.len()));
        }
        if items[0].label != "fn(function*(){})" {
            t.error(format!(
                "item[0].Label = {:?}, want {:?}",
                items[0].label, "fn(function*(){})"
            ));
        }
        if items[1].label != "fnUntraced(function*(){})" {
            t.error(format!(
                "item[1].Label = {:?}, want {:?}",
                items[1].label, "fnUntraced(function*(){})"
            ));
        }
    });
}

// go test: internal/completions/fn_function_star_test.go:65 TestFnFunctionStar_AliasedImport 8a1f3b3b3ddf4a2c
#[test]
fn test_fn_function_star_aliased_import() {
    go_test(|t| {
        t.parallel();
        let source = r#"import * as Fx from "effect/Effect"
const myFn = Fx.fn"#;
        let items = fn_function_star_items(t, source, source.len());
        if items.len() != 3 {
            t.fatal(format!("expected 3 items for aliased import, got {}", items.len()));
        }
        if items[0].label != r#"fn("myFn")"# {
            t.error(format!("item[0].Label = {:?}, want {:?}", items[0].label, r#"fn("myFn")"#));
        }
    });
}

// go test: internal/completions/fn_function_star_test.go:78 TestFnFunctionStar_CursorAfterDot dc2ed1521ccf0a2f
#[test]
fn test_fn_function_star_cursor_after_dot() {
    go_test(|t| {
        t.parallel();
        let source = r#"import * as Effect from "effect/Effect"
const myFn = Effect."#;
        let items = fn_function_star_items(t, source, source.len());
        if items.len() != 3 {
            t.fatal(format!("expected 3 items for cursor after dot, got {}", items.len()));
        }
    });
}

// go test: internal/completions/fn_function_star_test.go:88 TestFnFunctionStar_SnippetInsertText dbedb1cd4b84b8c0
#[test]
fn test_fn_function_star_snippet_insert_text() {
    go_test(|t| {
        t.parallel();
        let source = r#"import * as Effect from "effect/Effect"
const myFn = Effect.fn"#;
        let items = fn_function_star_items(t, source, source.len());
        if items.len() != 3 {
            t.fatal(format!("expected 3 items, got {}", items.len()));
        }

        let named_insert = new_text(&items[0]);
        let mut expected = r#"fn("myFn")(function*(${1}){${0}})"#;
        if named_insert != expected {
            t.error(format!("named insert text = {named_insert:?}, want {expected:?}"));
        }

        let generic_insert = new_text(&items[1]);
        expected = "fn(function*(${1}){${0}})";
        if generic_insert != expected {
            t.error(format!("generic insert text = {generic_insert:?}, want {expected:?}"));
        }

        let untraced_insert = new_text(&items[2]);
        expected = "fnUntraced(function*(${1}){${0}})";
        if untraced_insert != expected {
            t.error(format!("untraced insert text = {untraced_insert:?}, want {expected:?}"));
        }
    });
}
