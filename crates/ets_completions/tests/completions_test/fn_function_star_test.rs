use crate::*;

// go test: internal/completions/fn_function_star_test.go:7 TestFnFunctionStar_NotEffectModule 4de86fd5d7e65fe6
#[test]
fn test_fn_function_star_not_effect_module() {
    go_test(|t| {
        t.parallel();
        // Accessing a non-Effect identifier should return nil
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

// go test: internal/completions/fn_function_star_test.go:18 TestFnFunctionStar_NamespaceImport_InVarDecl 97fa0064c7c65cc6
#[test]
fn test_fn_function_star_namespace_import_in_var_decl() {
    go_test(|t| {
        t.parallel();
        // Effect.fn| inside a variable declaration should produce 3 items (named + generic + untraced)
        let source = r#"import * as Effect from "effect/Effect"
const myFn = Effect.fn"#;
        let items = fn_function_star_items(t, source, source.len());
        if items.len() != 3 {
            t.fatal(format!("expected 3 items, got {}", items.len()));
        }
        // First item should be the named variant with "myFn"
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

// go test: internal/completions/fn_function_star_test.go:39 TestFnFunctionStar_NamedImport_InVarDecl ccf7e8be23f8bdd1
#[test]
fn test_fn_function_star_named_import_in_var_decl() {
    go_test(|t| {
        t.parallel();
        // import { Effect } from "effect" — should also work
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

// go test: internal/completions/fn_function_star_test.go:53 TestFnFunctionStar_NotInVarDecl b56e257672a41bc1
#[test]
fn test_fn_function_star_not_in_var_decl() {
    go_test(|t| {
        t.parallel();
        // Effect.fn| not inside a variable declaration — should produce 2 items (no named variant)
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

// go test: internal/completions/fn_function_star_test.go:70 TestFnFunctionStar_AliasedImport f522e7c4855b6938
#[test]
fn test_fn_function_star_aliased_import() {
    go_test(|t| {
        t.parallel();
        // Aliased namespace import: import * as Fx from "effect/Effect" — Fx.fn| should work
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

// go test: internal/completions/fn_function_star_test.go:84 TestFnFunctionStar_CursorAfterDot 0bcf764c79fd05bf
#[test]
fn test_fn_function_star_cursor_after_dot() {
    go_test(|t| {
        t.parallel();
        // Cursor right after the dot (no partial text typed)
        let source = r#"import * as Effect from "effect/Effect"
const myFn = Effect."#;
        let items = fn_function_star_items(t, source, source.len());
        if items.len() != 3 {
            t.fatal(format!("expected 3 items for cursor after dot, got {}", items.len()));
        }
    });
}

// go test: internal/completions/fn_function_star_test.go:95 TestFnFunctionStar_SnippetInsertText 9a891228f860c4f3
#[test]
fn test_fn_function_star_snippet_insert_text() {
    go_test(|t| {
        t.parallel();
        // Verify the insert text contains proper snippet placeholders
        let source = r#"import * as Effect from "effect/Effect"
const myFn = Effect.fn"#;
        let items = fn_function_star_items(t, source, source.len());
        if items.len() != 3 {
            t.fatal(format!("expected 3 items, got {}", items.len()));
        }

        // Check insert text for named variant
        let named_insert = new_text(&items[0]);
        let mut expected = r#"fn("myFn")(function*(${1}){${0}})"#;
        if named_insert != expected {
            t.error(format!("named insert text = {named_insert:?}, want {expected:?}"));
        }

        // Check insert text for generic variant
        let generic_insert = new_text(&items[1]);
        expected = "fn(function*(${1}){${0}})";
        if generic_insert != expected {
            t.error(format!("generic insert text = {generic_insert:?}, want {expected:?}"));
        }

        // Check insert text for untraced variant
        let untraced_insert = new_text(&items[2]);
        expected = "fnUntraced(function*(${1}){${0}})";
        if untraced_insert != expected {
            t.error(format!("untraced insert text = {untraced_insert:?}, want {expected:?}"));
        }
    });
}
