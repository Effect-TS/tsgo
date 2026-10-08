package typeparser

import (
	"fmt"
	"strings"
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
)

const predicateTestImports = `
	import { Predicate } from "effect"
	import * as P from "effect/Predicate"
	import { isTagged as tagged } from "effect/Predicate"
	export { isTagged as reexported } from "effect/Predicate"
	import { reexported } from "./test.js"
	const alias = tagged
	const aliasChain = alias
	let mutable = tagged
	const applied = tagged("Foo")
	const fake = { isTagged: (_value: unknown, _tag: string) => true }
	type Tagged = { readonly _tag: "Foo" } | { readonly _tag: "Bar" }
	declare const expectedTag: "Foo" | "Bar"
	declare const args: [Tagged, "Foo"]
	declare const ready: boolean
`

func TestIsNodeReferenceToEffectPredicateModuleApi(t *testing.T) {
	t.Parallel()
	for _, version := range []bundledeffect.EffectVersion{bundledeffect.EffectV3, bundledeffect.EffectV4} {
		t.Run(string(version), func(t *testing.T) {
			t.Parallel()
			_, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectVersionInternal(t, version, predicateTestImports+`
				const root = (value: Tagged) => Predicate.isTagged(value, "Foo")
				const subpath = (value: Tagged) => P.isTagged(value, "Foo")
				const renamed = (value: Tagged) => tagged(value, "Foo")
				const exported = (value: Tagged) => reexported(value, "Foo")
				const constant = (value: Tagged) => aliasChain(value, "Foo")
				const mutableAlias = (value: Tagged) => mutable(value, "Foo")
				const lookalike = (value: Tagged) => fake.isTagged(value, "Foo")
				const shadowed = (Predicate: typeof fake) => Predicate.isTagged({}, "Foo")
				const otherExport = (value: Tagged) => P.hasProperty(value, "_tag")
			`)
			defer done()
			for _, tt := range []struct {
				name   string
				member string
				want   bool
			}{
				{name: "root", member: "isTagged", want: true},
				{name: "subpath", member: "isTagged", want: true},
				{name: "renamed", member: "isTagged", want: true},
				{name: "exported", member: "isTagged", want: true},
				{name: "constant", member: "isTagged", want: true},
				{name: "mutableAlias", member: "isTagged"},
				{name: "lookalike", member: "isTagged"},
				{name: "shadowed", member: "isTagged"},
				{name: "otherExport", member: "isTagged"},
				{name: "otherExport", member: "hasProperty", want: true},
			} {
				function := findReturningDispatchTestFunction(t, sf, tt.name)
				callee := GetFunctionLikeBody(function).AsCallExpression().Expression
				if got := tp.IsNodeReferenceToEffectPredicateModuleApi(callee, tt.member); got != tt.want {
					t.Errorf("%s as %s = %v, want %v", tt.name, tt.member, got, tt.want)
				}
			}
		})
	}
}

func TestParseTagMatchPredicate(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name       string
		expression string
		subject    string
		tag        string
	}{
		{name: "direct", expression: `Predicate.isTagged(value, "Foo")`, subject: "value", tag: `"Foo"`},
		{name: "curried", expression: `Predicate.isTagged("Foo")(value)`, subject: "value", tag: `"Foo"`},
		{name: "namespace", expression: `P.isTagged(value, "Foo")`, subject: "value", tag: `"Foo"`},
		{name: "renamed", expression: `tagged("Foo")(value)`, subject: "value", tag: `"Foo"`},
		{name: "reexported", expression: `reexported(value, "Foo")`, subject: "value", tag: `"Foo"`},
		{name: "constant alias", expression: `aliasChain("Foo")(value)`, subject: "value", tag: `"Foo"`},
		{name: "transparent direct", expression: `((Predicate.isTagged! as typeof tagged)((value as Tagged)!, ("Foo" satisfies string)))`, subject: "value", tag: `"Foo"`},
		{name: "transparent curried", expression: `((<typeof tagged>Predicate.isTagged)(("Foo" as const)) as (self: unknown) => boolean)((value satisfies Tagged)!)`, subject: "value", tag: `"Foo"`},
		{name: "nested subject", expression: `Predicate.isTagged({ reason: value }.reason, "Foo")`, subject: "{ reason: value }.reason", tag: `"Foo"`},
		{name: "dynamic direct tag", expression: `Predicate.isTagged(value, expectedTag)`, subject: "value", tag: "expectedTag"},
		{name: "dynamic curried tag", expression: `Predicate.isTagged(expectedTag)(value)`, subject: "value", tag: "expectedTag"},
		{name: "zero arguments", expression: `Predicate.isTagged()`},
		{name: "unapplied", expression: `Predicate.isTagged("Foo")`},
		{name: "extra direct argument", expression: `Predicate.isTagged(value, "Foo", ready)`},
		{name: "zero outer arguments", expression: `Predicate.isTagged("Foo")()`},
		{name: "extra outer argument", expression: `Predicate.isTagged("Foo")(value, other)`},
		{name: "extra inner argument", expression: `Predicate.isTagged("Foo", "Bar")(value)`},
		{name: "zero inner arguments", expression: `Predicate.isTagged()(value)`},
		{name: "spread subject", expression: `Predicate.isTagged(...args, "Foo")`},
		{name: "spread tag", expression: `Predicate.isTagged(value, ...["Foo"])`},
		{name: "spread inner", expression: `Predicate.isTagged(...["Foo"])(value)`},
		{name: "spread outer", expression: `Predicate.isTagged("Foo")(...[value])`},
		{name: "optional direct call", expression: `Predicate.isTagged?.(value, "Foo")`},
		{name: "optional module access", expression: `Predicate?.isTagged(value, "Foo")`},
		{name: "parenthesized optional access", expression: `(Predicate?.isTagged)(value, "Foo")`},
		{name: "optional inner call", expression: `Predicate.isTagged?.("Foo")(value)`},
		{name: "optional outer call", expression: `Predicate.isTagged("Foo")?.(value)`},
		{name: "negation", expression: `!Predicate.isTagged(value, "Foo")`},
		{name: "compound", expression: `Predicate.isTagged(value, "Foo") && ready`},
		{name: "boolean equality", expression: `Predicate.isTagged(value, "Foo") === true`},
		{name: "lookalike", expression: `fake.isTagged(value, "Foo")`},
		{name: "mutable alias", expression: `mutable(value, "Foo")`},
		{name: "applied predicate alias", expression: `applied(value)`},
		{name: "wrapper function", expression: `((self: Tagged, tag: "Foo") => tagged(self, tag))(value, "Foo")`},
		{name: "different export", expression: `Predicate.hasProperty(value, "_tag")`},
	}
	var source strings.Builder
	source.WriteString(predicateTestImports)
	for i, tt := range tests {
		fmt.Fprintf(&source, "\nconst match%d = (value: Tagged, other: Tagged) => %s\n", i, tt.expression)
	}
	for _, version := range []bundledeffect.EffectVersion{bundledeffect.EffectV3, bundledeffect.EffectV4} {
		t.Run(string(version), func(t *testing.T) {
			t.Parallel()
			_, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectVersionInternal(t, version, source.String())
			defer done()
			for i, tt := range tests {
				t.Run(tt.name, func(t *testing.T) {
					function := findReturningDispatchTestFunction(t, sf, fmt.Sprintf("match%d", i))
					body := GetFunctionLikeBody(function)
					subject, tag := tp.ParseTagMatch(body)
					if got := returningDispatchNodeText(sf, subject); got != tt.subject {
						t.Errorf("subject = %q, want %q", got, tt.subject)
					}
					if got := returningDispatchNodeText(sf, tag); got != tt.tag {
						t.Errorf("tag = %q, want %q", got, tt.tag)
					}
					for _, node := range []*ast.Node{subject, tag} {
						if node != nil && !predicateNodeDescendsFrom(node, body) {
							t.Error("tag evidence must preserve an original condition node")
						}
					}
				})
			}
		})
	}
}

func TestParseResultDispatchPredicateHints(t *testing.T) {
	t.Parallel()
	for _, version := range []bundledeffect.EffectVersion{bundledeffect.EffectV3, bundledeffect.EffectV4} {
		t.Run(string(version), func(t *testing.T) {
			t.Parallel()
			_, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectVersionInternal(t, version, predicateTestImports+`
				type Wrapper = { readonly reason: Tagged; readonly alternateReason: Tagged }
				declare const other: Wrapper
				const mixed = (value: Tagged) =>
					Predicate.isTagged(value, "Foo") ? 1 : value._tag === "Bar" ? 2 : 0
				const nested = (error: Wrapper) => {
					if (error.reason._tag === "Foo") return 1
					if (Predicate.isTagged("Bar")(error.reason)) return 2
					return 0
				}
				const wrapped = (value: Tagged) =>
					(Predicate.isTagged((value satisfies Tagged), ("Foo" as const))) ? 1 : Predicate.isTagged("Bar")(value) ? 2 : 0
				const differentRoot = (error: Wrapper) =>
					Predicate.isTagged(error.reason, "Foo") ? 1 : Predicate.isTagged(other.reason, "Bar") ? 2 : 0
				const differentProperty = (error: Wrapper) =>
					Predicate.isTagged(error.reason, "Foo") ? 1 : Predicate.isTagged(error.alternateReason, "Bar") ? 2 : 0
				const partlyUntagged = (value: Tagged) =>
					Predicate.isTagged(value, "Foo") ? 1 : fake.isTagged(value, "Bar") ? 2 : 0
			`)
			defer done()
			for _, tt := range []struct {
				name    string
				subject string
			}{
				{name: "mixed", subject: "value"},
				{name: "nested", subject: "error.reason"},
				{name: "wrapped", subject: "value"},
				{name: "differentRoot"},
				{name: "differentProperty"},
				{name: "partlyUntagged"},
			} {
				function := findReturningDispatchTestFunction(t, sf, tt.name)
				returning := tp.ParseReturningDispatch(function)
				if returning == nil {
					t.Fatalf("%s: expected returning dispatch", tt.name)
				}
				for _, dispatch := range []*ResultDispatch{returning.Dispatch, tp.ParseResultDispatch(GetFunctionLikeBody(function))} {
					if got := returningDispatchNodeText(sf, dispatch.CommonTagSubject(tp)); got != tt.subject {
						t.Errorf("%s: common subject = %q, want %q", tt.name, got, tt.subject)
					}
					for _, branch := range dispatch.Branches {
						condition := branch.Condition
						if condition.Source != condition.Subject || condition.Value != nil ||
							!predicateNodeDescendsFrom(condition.Source, GetFunctionLikeBody(function)) {
							t.Errorf("%s: predicate source evidence changed", tt.name)
						}
						if condition.TagSubject != nil &&
							(!predicateNodeDescendsFrom(condition.TagSubject, condition.Source) || !predicateNodeDescendsFrom(condition.TagValue, condition.Source)) {
							t.Errorf("%s: tag evidence is not from the original condition", tt.name)
						}
					}
				}
			}
			for _, name := range []string{"mixed", "wrapped"} {
				function := findReturningDispatchTestFunction(t, sf, name)
				original := GetFunctionLikeBody(function).AsConditionalExpression().Condition
				condition := tp.ParseReturningDispatch(function).Dispatch.Branches[0].Condition
				call := unwrapResultDispatchExpression(original).AsCallExpression()
				if condition.Source != original || condition.Subject != original ||
					condition.TagSubject != unwrapResultDispatchExpression(call.Arguments.Nodes[0]) ||
					condition.TagValue != unwrapResultDispatchExpression(call.Arguments.Nodes[1]) {
					t.Errorf("%s: Predicate call must preserve the exact condition and unwrapped argument nodes", name)
				}
			}
		})
	}
}

func predicateNodeDescendsFrom(node *ast.Node, ancestor *ast.Node) bool {
	for ; node != nil; node = node.Parent {
		if node == ancestor {
			return true
		}
	}
	return false
}
