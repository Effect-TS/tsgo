// Ported from effect-tsgo's internal/typeparser/uses_function_own_bindings_test.go.

use ets_go::testing::T;
use ets_shim::ast::{self, Kind, Node, SourceFile};
use ets_shim::core::P;

use crate::helpers_test::*;

// go test: internal/typeparser/uses_function_own_bindings_test.go:11 ownBindingsFunction 82bd5efb0ccdce7e
// ownBindingsFunction returns the function declared as fn: the initializer of
// a variable fn, or a function or method named fn.
fn own_bindings_function(t: &T, sf: P<SourceFile>) -> P<Node> {
    t.helper();
    let declaration =
        find_identifier_by_text(t, sf, "fn", 0).parent().expect("an identifier has a parent");
    match declaration.kind() {
        Kind::VariableDeclaration => {
            ast::skip_parentheses(declaration.initializer().expect("fn has an initializer"))
        }
        Kind::FunctionDeclaration | Kind::MethodDeclaration => declaration,
        kind => t.fatal(format!("unexpected declaration of fn: {kind:?}")),
    }
}

// go test: internal/typeparser/uses_function_own_bindings_test.go:24 TestUsesFunctionOwnBindings 25471d7dc078eda2
#[test]
fn test_uses_function_own_bindings() {
    run(|t| {
        t.parallel();

        #[rustfmt::skip]
        let tests: &[(&str, &str, bool)] = &[
        ("arrow", r"export function outer(this: unknown) {
  const fn = () => [this, arguments, new.target]
  return fn
}", false),
        ("this", r"export const fn = function(this: unknown) { return this }", true),
        ("super", r"export const o = { fn() { return super.toString() } }", true),
        ("new.target", r"export const fn = function() { return new.target }", true),
        ("arguments", r"export const fn = function() { return arguments.length }", true),
        ("this in nested arrow", r"export const fn = function(this: unknown) { return () => this }", true),
        ("super in nested arrow", r"export const o = { fn() { return () => super.toString() } }", true),
        ("new.target in nested arrow", r"export const fn = function() { return () => new.target }", true),
        ("arguments in nested arrow", r"export const fn = function() { return () => arguments.length }", true),
        ("nested function", r"export const fn = function() {
  return function(this: unknown) { return [this, new.target, arguments.length] }
}", false),
        ("nested function declaration", r"export const fn = function() {
  function inner(this: unknown) { return [this, new.target, arguments.length] }
  return inner
}", false),
        ("nested method", r"export const fn = function() {
  return { m() { return [this, super.toString(), arguments.length] } }
}", false),
        ("nested accessor", r"export const fn = function() {
  return { get g() { return [this, arguments.length] }, set s(_: number) { void this } }
}", false),
        ("nested constructor", r"export const fn = function() {
  return class { constructor() { void [this, new.target, arguments.length] } }
}", false),
        ("nested class field", r"export const fn = function() {
  return class { readonly self = this; readonly length = arguments.length }
}", false),
        ("nested static block", r"export const fn = function() {
  return class { static { void [this, arguments.length] } }
}", false),
        ("computed property name", r"export const fn = function(this: { x: string }) { return { [this.x]: 1 } }", true),
        ("computed class method name", r"export const fn = function(this: { x: string }) { return class { [this.x]() {} } }", true),
        ("computed class field name", r"export const fn = function() { return class { [String(arguments.length)] = 1 } }", true),
        ("computed name in nested function", r"export const fn = function() {
  return function(this: { x: string }) { return { [this.x]: 1 } }
}", false),
        ("member decorator", r"declare const dec: (value: unknown) => any
export const fn = function(this: unknown) { return class { @dec(this) m() {} } }", true),
        ("parameter decorator", r"declare const dec: (value: unknown) => any
export const fn = function(this: unknown) { return class { m(@dec(this) _p: number) {} } }", true),
        ("constructor parameter decorator", r"declare const dec: (value: unknown) => any
export const fn = function() { return class { constructor(@dec(new.target) _p: number) {} } }", true),
        ("extends", r"export const fn = function(this: { Base: new() => object }) { return class extends this.Base {} }", true),
        ("shorthand arguments", r"export const fn = function() { return { arguments } }", true),
        ("property named arguments", r"export const fn = function(obj: { arguments: number }) { return obj.arguments }", false),
        ("import.meta", r"export const fn = function() { return import.meta.url }", false),
        ("shadowing arguments function", r"export const fn = function() {
  function arguments() { return 1 }
  return arguments()
}", false),
        ("own name", r"export const fn = function self() { return self.length }", true),
        ("own name in nested arrow", r"export const fn = function self() { return () => self.length }", true),
        ("own name in nested function", r"export const fn = function self() { return function() { return self.length } }", true),
        ("local shadowing own name", r"export const fn = function self() {
  const self = 1
  return self
}", false),
        ("function declaration name", r"export function fn(): unknown { return fn }", false),
        ("nothing", r"export const fn = function(a: number) { return [a, (b: number) => b + 1] }", false),
        ];

        for &(name, source, want) in tests {
            t.run(name, |t| {
                t.parallel();

                let (mut compiled, sf) =
                    compile_and_get_checker_and_source_file_internal(t, source);
                let mut tp = compiled.type_parser();

                let fn_ = own_bindings_function(t, sf);
                let got = tp.uses_function_own_bindings(fn_, fn_.body());
                if got != want {
                    t.fatal(format!("UsesFunctionOwnBindings = {got}, want {want}"));
                }
            });
        }
    });
}

// go test: internal/typeparser/uses_function_own_bindings_test.go:119 TestUsesFunctionOwnBindings_ParameterRoot 314fed100c2757fa
// TestUsesFunctionOwnBindings_ParameterRoot checks a root that is a parameter
// of fn rather than its body: a default value reads fn's arguments.
#[test]
fn test_uses_function_own_bindings_parameter_root() {
    run(|t| {
        t.parallel();

        let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(
            t,
            "export const fn = function(a = arguments.length, b = 1) { return [a, b] }",
        );
        let mut tp = compiled.type_parser();

        let fn_ = own_bindings_function(t, sf);
        let parameters = fn_.parameters();
        if !tp.uses_function_own_bindings(fn_, parameters[0]) {
            t.fatal("expected the first parameter to read fn's arguments");
        }
        if tp.uses_function_own_bindings(fn_, parameters[1]) {
            t.fatal("expected the second parameter to read no binding of fn");
        }
    });
}

// go test: internal/typeparser/uses_function_own_bindings_test.go:137 TestUsesFunctionOwnBindings_RootInNestedFunction 617253e3d28e3a31
// TestUsesFunctionOwnBindings_RootInNestedFunction checks a root inside a nested
// function of fn: its this, new.target and arguments belong to the nested one.
#[test]
fn test_uses_function_own_bindings_root_in_nested_function() {
    run(|t| {
        t.parallel();

        let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(
            t,
            r"export const fn = function self() {
  return function inner(this: unknown) { return [this, new.target, arguments.length] }
}
export const named = function self() {
  return function inner() { return self.length }
}",
        );
        let mut tp = compiled.type_parser();

        let inner =
            find_identifier_by_text(t, sf, "inner", 0).parent().expect("inner has a parent");
        if tp.uses_function_own_bindings(own_bindings_function(t, sf), inner.body()) {
            t.fatal("expected the nested function's keywords not to belong to fn");
        }
        let named = ast::skip_parentheses(
            find_identifier_by_text(t, sf, "named", 0)
                .parent()
                .and_then(|parent| parent.initializer())
                .expect("named has an initializer"),
        );
        let named_inner =
            find_identifier_by_text(t, sf, "inner", 1).parent().expect("inner has a parent");
        if !tp.uses_function_own_bindings(named, named_inner.body()) {
            t.fatal("expected fn's own name to be read inside the nested function");
        }
    });
}
