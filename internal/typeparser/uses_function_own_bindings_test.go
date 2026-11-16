package typeparser

import (
	"testing"

	"github.com/microsoft/TypeScript/tsc/shim/ast"
)

// ownBindingsFunction returns the function declared as fn: the initializer of
// a variable fn, or a function or method named fn.
func ownBindingsFunction(t *testing.T, sf *ast.SourceFile) *ast.Node {
	t.Helper()
	declaration := findIdentifierByText(t, sf, "fn", 0).Parent
	switch declaration.Kind {
	case ast.KindVariableDeclaration:
		return ast.SkipParentheses(declaration.Initializer())
	case ast.KindFunctionDeclaration, ast.KindMethodDeclaration:
		return declaration
	}
	t.Fatalf("unexpected declaration of fn: %v", declaration.Kind)
	return nil
}

func TestUsesFunctionOwnBindings(t *testing.T) {
	t.Parallel()

	tests := []struct {
		name   string
		source string
		want   bool
	}{
		{"arrow", `export function outer(this: unknown) {
  const fn = () => [this, arguments, new.target]
  return fn
}`, false},

		{"this", `export const fn = function(this: unknown) { return this }`, true},
		{"super", `export const o = { fn() { return super.toString() } }`, true},
		{"new.target", `export const fn = function() { return new.target }`, true},
		{"arguments", `export const fn = function() { return arguments.length }`, true},
		{"this in nested arrow", `export const fn = function(this: unknown) { return () => this }`, true},
		{"super in nested arrow", `export const o = { fn() { return () => super.toString() } }`, true},
		{"new.target in nested arrow", `export const fn = function() { return () => new.target }`, true},
		{"arguments in nested arrow", `export const fn = function() { return () => arguments.length }`, true},

		{"nested function", `export const fn = function() {
  return function(this: unknown) { return [this, new.target, arguments.length] }
}`, false},
		{"nested function declaration", `export const fn = function() {
  function inner(this: unknown) { return [this, new.target, arguments.length] }
  return inner
}`, false},
		{"nested method", `export const fn = function() {
  return { m() { return [this, super.toString(), arguments.length] } }
}`, false},
		{"nested accessor", `export const fn = function() {
  return { get g() { return [this, arguments.length] }, set s(_: number) { void this } }
}`, false},
		{"nested constructor", `export const fn = function() {
  return class { constructor() { void [this, new.target, arguments.length] } }
}`, false},
		{"nested class field", `export const fn = function() {
  return class { readonly self = this; readonly length = arguments.length }
}`, false},
		{"nested static block", `export const fn = function() {
  return class { static { void [this, arguments.length] } }
}`, false},

		{"computed property name", `export const fn = function(this: { x: string }) { return { [this.x]: 1 } }`, true},
		{"computed class method name", `export const fn = function(this: { x: string }) { return class { [this.x]() {} } }`, true},
		{"computed class field name", `export const fn = function() { return class { [String(arguments.length)] = 1 } }`, true},
		{"computed name in nested function", `export const fn = function() {
  return function(this: { x: string }) { return { [this.x]: 1 } }
}`, false},
		{"member decorator", `declare const dec: (value: unknown) => any
export const fn = function(this: unknown) { return class { @dec(this) m() {} } }`, true},
		{"parameter decorator", `declare const dec: (value: unknown) => any
export const fn = function(this: unknown) { return class { m(@dec(this) _p: number) {} } }`, true},
		{"constructor parameter decorator", `declare const dec: (value: unknown) => any
export const fn = function() { return class { constructor(@dec(new.target) _p: number) {} } }`, true},
		{"extends", `export const fn = function(this: { Base: new() => object }) { return class extends this.Base {} }`, true},

		{"shorthand arguments", `export const fn = function() { return { arguments } }`, true},
		{"property named arguments", `export const fn = function(obj: { arguments: number }) { return obj.arguments }`, false},
		{"import.meta", `export const fn = function() { return import.meta.url }`, false},
		{"shadowing arguments function", `export const fn = function() {
  function arguments() { return 1 }
  return arguments()
}`, false},

		{"own name", `export const fn = function self() { return self.length }`, true},
		{"own name in nested arrow", `export const fn = function self() { return () => self.length }`, true},
		{"own name in nested function", `export const fn = function self() { return function() { return self.length } }`, true},
		{"local shadowing own name", `export const fn = function self() {
  const self = 1
  return self
}`, false},
		{"function declaration name", `export function fn(): unknown { return fn }`, false},
		{"nothing", `export const fn = function(a: number) { return [a, (b: number) => b + 1] }`, false},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()

			_, tp, sf, done := compileAndGetCheckerAndSourceFileInternal(t, test.source)
			defer done()

			fn := ownBindingsFunction(t, sf)
			if got := tp.UsesFunctionOwnBindings(fn, fn.Body()); got != test.want {
				t.Fatalf("UsesFunctionOwnBindings = %v, want %v", got, test.want)
			}
		})
	}
}

// TestUsesFunctionOwnBindings_ParameterRoot checks a root that is a parameter
// of fn rather than its body: a default value reads fn's arguments.
func TestUsesFunctionOwnBindings_ParameterRoot(t *testing.T) {
	t.Parallel()

	_, tp, sf, done := compileAndGetCheckerAndSourceFileInternal(t, `export const fn = function(a = arguments.length, b = 1) { return [a, b] }`)
	defer done()

	fn := ownBindingsFunction(t, sf)
	parameters := fn.Parameters()
	if !tp.UsesFunctionOwnBindings(fn, parameters[0]) {
		t.Fatal("expected the first parameter to read fn's arguments")
	}
	if tp.UsesFunctionOwnBindings(fn, parameters[1]) {
		t.Fatal("expected the second parameter to read no binding of fn")
	}
}

// TestUsesFunctionOwnBindings_RootInNestedFunction checks a root inside a nested
// function of fn: its this, new.target and arguments belong to the nested one.
func TestUsesFunctionOwnBindings_RootInNestedFunction(t *testing.T) {
	t.Parallel()

	_, tp, sf, done := compileAndGetCheckerAndSourceFileInternal(t, `export const fn = function self() {
  return function inner(this: unknown) { return [this, new.target, arguments.length] }
}
export const named = function self() {
  return function inner() { return self.length }
}`)
	defer done()

	inner := findIdentifierByText(t, sf, "inner", 0).Parent
	if tp.UsesFunctionOwnBindings(ownBindingsFunction(t, sf), inner.Body()) {
		t.Fatal("expected the nested function's keywords not to belong to fn")
	}
	named := ast.SkipParentheses(findIdentifierByText(t, sf, "named", 0).Parent.Initializer())
	namedInner := findIdentifierByText(t, sf, "inner", 1).Parent
	if !tp.UsesFunctionOwnBindings(named, namedInner.Body()) {
		t.Fatal("expected fn's own name to be read inside the nested function")
	}
}
