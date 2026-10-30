package typeparser

import (
	"slices"
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

func firstNodeOfKind(t *testing.T, sf *ast.SourceFile, kind ast.Kind) *ast.Node {
	t.Helper()
	var found *ast.Node
	var visit func(node *ast.Node) bool
	visit = func(node *ast.Node) bool {
		if found == nil && node.Kind == kind {
			found = node
		}
		node.ForEachChild(visit)
		return found != nil
	}
	sf.AsNode().ForEachChild(visit)
	if found == nil {
		t.Fatalf("no %v found", kind)
	}
	return found
}

func TestNodeCannotBeEffectLibraryTypeObjectLiteralsOnlyInV4(t *testing.T) {
	t.Parallel()
	source := `import { Effect } from "effect"
export const value = { a: 1 }
export const effect = Effect.void`
	for _, tt := range []struct {
		version  bundledeffect.EffectVersion
		expected bool
	}{
		{bundledeffect.EffectV4, true},
		{bundledeffect.EffectV3, false},
	} {
		if err := bundledeffect.EnsurePackageInstalled(tt.version, "effect"); err != nil {
			t.Skip("Effect not installed:", err)
		}
		_, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectVersionInternal(t, tt.version, source)
		if got := tp.nodeCannotBeEffectLibraryType(firstNodeOfKind(t, sf, ast.KindObjectLiteralExpression)); got != tt.expected {
			t.Errorf("%v: nodeCannotBeEffectLibraryType(object literal) = %v, want %v", tt.version, got, tt.expected)
		}
		done()
	}
}

func TestNodeCouldBeEffectKeepsNarrowedReferences(t *testing.T) {
	t.Parallel()
	if err := bundledeffect.EnsurePackageInstalled(bundledeffect.EffectV4, "effect"); err != nil {
		t.Skip("Effect v4 not installed:", err)
	}
	source := `
import { Effect } from "effect"
interface Plain { readonly value: number }
declare const guarded: Plain
export const narrowed = Effect.isEffect(guarded) ? guarded : null
`
	_, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectV4Internal(t, source)
	defer done()

	narrowed := firstNodeOfKind(t, sf, ast.KindConditionalExpression).AsConditionalExpression().WhenTrue
	if tp.EffectType(tp.GetTypeAtLocation(narrowed)) == nil {
		t.Fatal("fixture: the guarded reference should narrow to an Effect")
	}
	if !tp.NodeCouldBeEffect(narrowed) {
		t.Error("a reference narrowed by a type guard must not be ruled out")
	}
}

func TestNodeCannotBeEffectLibraryTypePrimitiveExpressions(t *testing.T) {
	t.Parallel()
	if err := bundledeffect.EnsurePackageInstalled(bundledeffect.EffectV4, "effect"); err != nil {
		t.Skip("Effect v4 not installed:", err)
	}
	source := `import { Effect } from "effect"
declare const x: number
export const trueLiteral = true
export const falseLiteral = false
export const nullLiteral = null
export const regex = /a/
export const not = !x
export const negate = -x
export const voided = void 0
export const typeOf = typeof x
export const undefinedReference = undefined
export const effect = Effect.void`
	_, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectV4Internal(t, source)
	defer done()

	initializers := exportedInitializers(sf)
	for name, expected := range map[string]bool{
		"trueLiteral":        true,
		"falseLiteral":       true,
		"nullLiteral":        true,
		"regex":              true,
		"not":                true,
		"negate":             true,
		"voided":             true,
		"typeOf":             true,
		"undefinedReference": false,
		"effect":             false,
	} {
		node := initializers[name]
		if got := tp.nodeCannotBeEffectLibraryType(node); got != expected {
			t.Errorf("nodeCannotBeEffectLibraryType(%s) = %v, want %v", name, got, expected)
		}
		if isEffect := tp.EffectType(tp.GetTypeAtLocation(node)) != nil; expected && isEffect {
			t.Errorf("fixture: %s was ruled out but its type is an Effect", name)
		}
	}
}

type atLocationParser struct {
	name       string
	atLocation func(node *ast.Node) (any, bool)
	parse      func(t *checker.Type) (any, bool)
}

func atLocationParsers(tp *TypeParser) []atLocationParser {
	return []atLocationParser{
		{"Effect", func(n *ast.Node) (any, bool) { r := tp.EffectTypeAtLocation(n); return r, r != nil },
			func(t *checker.Type) (any, bool) { r := tp.EffectType(t); return r, r != nil }},
		{"StrictEffect", func(n *ast.Node) (any, bool) { r := tp.StrictEffectTypeAtLocation(n); return r, r != nil },
			func(t *checker.Type) (any, bool) { r := tp.StrictEffectType(t); return r, r != nil }},
		{"EffectTypeId", func(n *ast.Node) (any, bool) { r := tp.HasEffectTypeIdAtLocation(n); return r, r },
			func(t *checker.Type) (any, bool) { r := tp.HasEffectTypeId(t); return r, r }},
		{"Stream", func(n *ast.Node) (any, bool) { r := tp.StreamTypeAtLocation(n); return r, r != nil },
			func(t *checker.Type) (any, bool) { r := tp.StreamType(t); return r, r != nil }},
		{"Layer", func(n *ast.Node) (any, bool) { r := tp.LayerTypeAtLocation(n); return r, r != nil },
			func(t *checker.Type) (any, bool) { r := tp.LayerType(t); return r, r != nil }},
		{"Schema", func(n *ast.Node) (any, bool) { r := tp.EffectSchemaTypesAtLocation(n); return r, r != nil },
			func(t *checker.Type) (any, bool) { r := tp.EffectSchemaTypes(t); return r, r != nil }},
		{"Service", func(n *ast.Node) (any, bool) { r := tp.ServiceTypeAtLocation(n); return r, r != nil },
			func(t *checker.Type) (any, bool) { r := tp.ServiceType(t); return r, r != nil }},
		{"ContextTag", func(n *ast.Node) (any, bool) { r := tp.ContextTagAtLocation(n); return r, r != nil },
			func(t *checker.Type) (any, bool) { r := tp.ContextTag(t); return r, r != nil }},
		{"Yieldable", func(n *ast.Node) (any, bool) { r := tp.EffectYieldableTypeAtLocation(n); return r, r != nil },
			func(t *checker.Type) (any, bool) { r := tp.EffectYieldableType(t); return r, r != nil }},
	}
}

func exportedInitializers(sf *ast.SourceFile) map[string]*ast.Node {
	initializers := map[string]*ast.Node{}
	for _, statement := range sf.Statements.Nodes {
		if statement.Kind != ast.KindVariableStatement {
			continue
		}
		for _, declaration := range statement.AsVariableStatement().DeclarationList.AsVariableDeclarationList().Declarations.Nodes {
			initializers[declaration.Name().Text()] = declaration.Initializer()
		}
	}
	return initializers
}

func checkAtLocationParsers(t *testing.T, tp *TypeParser, sf *ast.SourceFile, expected map[string][]string) {
	t.Helper()
	initializers := exportedInitializers(sf)
	parsers := atLocationParsers(tp)
	for name, accepted := range expected {
		node := initializers[name]
		if node == nil {
			t.Fatalf("no initializer for %s", name)
		}
		results := map[string]any{}
		for _, parser := range parsers {
			result, ok := parser.atLocation(node)
			results[parser.name] = result
			if want := slices.Contains(accepted, parser.name); ok != want {
				t.Errorf("%s: %sAtLocation succeeded = %v, want %v", name, parser.name, ok, want)
			}
		}
		if tp.nodeCannotBeEffectLibraryType(node) && node.Kind != ast.KindObjectLiteralExpression && tp.links.TypeAtLocation.Has(node) {
			t.Errorf("%s: a ruled-out node was typed", name)
		}
		for _, parser := range parsers {
			if want, _ := parser.parse(tp.GetTypeAtLocation(node)); results[parser.name] != want {
				t.Errorf("%s: %sAtLocation differs from the parser on GetTypeAtLocation", name, parser.name)
			}
		}
	}
}

func TestAtLocationParsersV4(t *testing.T) {
	t.Parallel()
	if err := bundledeffect.EnsurePackageInstalled(bundledeffect.EffectV4, "effect"); err != nil {
		t.Skip("Effect v4 not installed:", err)
	}
	_, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectV4Internal(t, `import { Context, Effect, Layer, Schema, Stream } from "effect"
class UserRepo extends Context.Service<UserRepo, { readonly find: () => string }>()("UserRepo") {}
export const effect = Effect.succeed(1)
export const layer = Layer.empty
export const stream = Stream.make(1)
export const schema = Schema.String
export const service = UserRepo
export const yieldable = { asEffect: () => Effect.succeed(1) }
export const object = { value: 1 }
export const arrow = () => Effect.succeed(1)
export const number = 1`)
	defer done()

	checkAtLocationParsers(t, tp, sf, map[string][]string{
		"effect":    {"Effect", "EffectTypeId", "StrictEffect", "Yieldable"},
		"layer":     {"Layer"},
		"stream":    {"Stream"},
		"schema":    {"Schema"},
		"service":   {"Service", "Effect", "EffectTypeId", "Yieldable"},
		"yieldable": {"Yieldable"},
		"object":    nil,
		"arrow":     nil,
		"number":    nil,
	})
}

func TestAtLocationParsersV3(t *testing.T) {
	t.Parallel()
	if err := bundledeffect.EnsurePackageInstalled(bundledeffect.EffectV3, "effect"); err != nil {
		t.Skip("Effect v3 not installed:", err)
	}
	_, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectV3Internal(t, `import { Context, Effect, Layer, Schema, Stream } from "effect"
class Config extends Context.Tag("Config")<Config, { readonly port: number }>() {}
export const effect = Effect.succeed(1)
export const layer = Layer.empty
export const stream = Stream.make(1)
export const schema = Schema.String
export const tag = Config
export const object = { value: 1 }
export const arrow = () => Effect.succeed(1)
export const number = 1`)
	defer done()

	v3StreamAlsoParsesAsEffect := []string{"Effect", "EffectTypeId", "Stream", "Yieldable"}
	checkAtLocationParsers(t, tp, sf, map[string][]string{
		"effect": append([]string{"StrictEffect"}, v3StreamAlsoParsesAsEffect...),
		"layer":  {"Layer"},
		"stream": v3StreamAlsoParsesAsEffect,
		"schema": {"Schema"},
		"tag":    append([]string{"ContextTag"}, v3StreamAlsoParsesAsEffect...),
		"object": nil,
		"arrow":  nil,
		"number": nil,
	})
}

func TestAtLocationParsersAreNilSafe(t *testing.T) {
	t.Parallel()
	var nilParser *TypeParser
	for _, tp := range []*TypeParser{nilParser, {}} {
		for _, parser := range atLocationParsers(tp) {
			if _, ok := parser.atLocation(nil); ok {
				t.Errorf("%sAtLocation(nil) succeeded", parser.name)
			}
		}
	}
	if _, ok := atLocationParsers(nilParser)[0].atLocation(&ast.Node{Kind: ast.KindIdentifier}); ok {
		t.Error("EffectTypeAtLocation on a nil TypeParser succeeded")
	}
}
