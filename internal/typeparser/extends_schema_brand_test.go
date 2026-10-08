package typeparser

import (
	"fmt"
	"strings"
	"testing"

	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

func TestExtendsSchemaBrandSyntax(t *testing.T) {
	t.Parallel()
	constructors := []struct {
		member, innerArgs, outerArgs string
	}{
		{"Class", `"schema-key"`, `{ value: Schema.String }`},
		{"TaggedClass", `"schema-key"`, `"schema-tag", { value: Schema.String }`},
		{"Error", `"schema-key"`, `{ value: Schema.String }`},
		{"TaggedError", `"schema-key"`, `"schema-tag", { value: Schema.String }`},
		{"Opaque", "", "Schema.Struct({ value: Schema.String })"},
	}
	brands := []struct {
		name, text string
		kind       ast.Kind
	}{
		{"Unbranded", "", ast.KindUnknown},
		{"Literal", `"Brand"`, ast.KindLiteralType},
		{"UniqueSymbol", `{ readonly brand: unique symbol }`, ast.KindTypeLiteral},
		{"Alias", "BrandAlias", ast.KindTypeReference},
		{"Empty", "{}", ast.KindTypeLiteral},
	}
	imports := []struct{ name, prefix string }{
		{"Namespace", "Schema."},
		{"ModuleNamespace", "S."},
		{"Renamed", "Make"},
	}
	var source strings.Builder
	source.WriteString(`
import { Schema } from "effect"
import * as S from "effect/Schema"
import { Class as MakeClass, TaggedClass as MakeTaggedClass, Error as MakeError,
  TaggedError as MakeTaggedError, Opaque as MakeOpaque } from "effect/Schema"
type BrandAlias = { readonly brand: unique symbol }
const schemaKey = "dynamic-key"
const schemaTag = "dynamic-tag"
`)
	source.WriteString("namespace Local {\n")
	for _, constructor := range constructors {
		fmt.Fprintf(&source, "export const %s = <Self, Brand = {}>(...args: unknown[]) => (...fields: unknown[]) => class {}\n", constructor.member)
	}
	source.WriteString("}\n")
	type testCase struct {
		name, member, brand, key, tag string
		brandKind                     ast.Kind
		match                         bool
	}
	var tests []testCase
	for _, constructor := range constructors {
		member := constructor.member
		add := func(suffix, expression, brand string, kind ast.Kind, match bool, key, tag string) {
			name := member + suffix
			expression = strings.ReplaceAll(expression, "$Self", name)
			fmt.Fprintf(&source, "class %s extends %s {}\n", name, expression)
			tests = append(tests, testCase{name, member, brand, key, tag, kind, match})
		}
		key, tag := "", ""
		if member == "TaggedClass" || member == "TaggedError" {
			key, tag = `"schema-key"`, `"schema-tag"`
		}
		for _, imported := range imports {
			for _, brand := range brands {
				typeArgs := "$Self"
				if brand.text != "" {
					typeArgs += ", " + brand.text
				}
				expression := fmt.Sprintf("%s%s<%s>(%s)(%s)", imported.prefix, member, typeArgs, constructor.innerArgs, constructor.outerArgs)
				add(imported.name+brand.name, expression, brand.text, brand.kind, true, key, tag)
			}
		}
		for _, invalid := range []struct{ name, expression string }{
			{"MissingSelf", fmt.Sprintf("Schema.%s(%s)(%s)", member, constructor.innerArgs, constructor.outerArgs)},
			{"SingleCall", fmt.Sprintf("Schema.%s<$Self>(%s)", member, constructor.innerArgs)},
			{"Lookalike", fmt.Sprintf("Local.%s<$Self, BrandAlias>(%s)(%s)", member, constructor.innerArgs, constructor.outerArgs)},
		} {
			add(invalid.name, invalid.expression, "", ast.KindUnknown, false, "", "")
		}
		add("ThreeArgs", fmt.Sprintf("Schema.%s<$Self, BrandAlias, {}>(%s)(%s)", member, constructor.innerArgs, constructor.outerArgs), "BrandAlias", ast.KindTypeReference, member != "Opaque", key, tag)
		if key != "" {
			add("DynamicLiterals", fmt.Sprintf("Schema.%s<$Self, {}>(schemaKey)(schemaTag, { value: Schema.String })", member), "{}", ast.KindTypeLiteral, true, "", "")
			add("OmittedKey", fmt.Sprintf("Schema.%s<$Self>()(\"schema-tag\", { value: Schema.String })", member), "", ast.KindUnknown, true, "", tag)
		}
	}
	c, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectV4Internal(t, source.String())
	t.Cleanup(done)
	parsers := map[string]func(*ast.Node) any{
		"Class":       func(node *ast.Node) any { return tp.ExtendsSchemaClass(node) },
		"TaggedClass": func(node *ast.Node) any { return tp.ExtendsSchemaTaggedClass(node) },
		"Error":       func(node *ast.Node) any { return tp.ExtendsSchemaError(node) },
		"TaggedError": func(node *ast.Node) any { return tp.ExtendsSchemaTaggedError(node) },
		"Opaque":      func(node *ast.Node) any { return tp.ExtendsSchemaOpaque(node) },
	}
	for _, test := range tests {
		name, _ := findClassTypeByName(t, c, sf, test.name)
		class := name.Parent
		if test.member == "Class" && tp.ExtendsSchemaError(class) != nil {
			t.Fatal("Class must not match Error")
		}
		if test.member == "Error" && tp.ExtendsSchemaClass(class) != nil {
			t.Fatal("Error must not match Class")
		}
		parse := parsers[test.member]
		got := parse(class)
		if parse(class) != got {
			t.Fatal("repeated parse did not return the cached result pointer")
		}
		var self, brand, className, key, tag *ast.Node
		var matched bool
		switch result := got.(type) {
		case *SchemaClassResult:
			matched = result != nil
			if matched {
				self, brand, className = result.SelfTypeNode, result.BrandTypeNode, result.ClassName
			}
		case *SchemaTaggedResult:
			matched = result != nil
			if matched {
				self, brand, className = result.SelfTypeNode, result.BrandTypeNode, result.ClassName
				key, tag = result.KeyStringLiteral, result.TagStringLiteral
			}
		case *SchemaOpaqueResult:
			matched = result != nil
			if matched {
				self, brand = result.SelfTypeNode, result.BrandTypeNode
			}
		default:
			t.Fatalf("unexpected parser result %T", got)
		}
		if matched != test.match {
			t.Fatalf("matched = %v, want %v", matched, test.match)
		}
		if !test.match {
			continue
		}
		assertSchemaExtendsNode(t, sf, self, ast.KindTypeReference, test.name)
		assertSchemaExtendsNode(t, sf, brand, test.brandKind, test.brand)
		if test.member != "Opaque" {
			assertSchemaExtendsNode(t, sf, className, ast.KindIdentifier, test.name)
			if className != name {
				t.Fatal("ClassName is not the original class identifier")
			}
		}
		assertSchemaExtendsNode(t, sf, key, ast.KindStringLiteral, test.key)
		assertSchemaExtendsNode(t, sf, tag, ast.KindStringLiteral, test.tag)
		outer := ast.GetExtendsHeritageClauseElements(class)[0].AsExpressionWithTypeArguments().Expression.AsCallExpression()
		inner := outer.Expression.AsCallExpression()
		if self != inner.TypeArguments.Nodes[0] {
			t.Fatal("SelfTypeNode is not the original first type argument")
		}
		if test.brand != "" && brand != inner.TypeArguments.Nodes[1] {
			t.Fatal("BrandTypeNode is not the original second type argument")
		}
		if key != nil && key != inner.Arguments.Nodes[0] {
			t.Fatal("KeyStringLiteral is not the original identifier argument")
		}
		if tag != nil && tag != outer.Arguments.Nodes[0] {
			t.Fatal("TagStringLiteral is not the original tag argument")
		}
	}
}

func assertSchemaExtendsNode(t *testing.T, sf *ast.SourceFile, node *ast.Node, kind ast.Kind, text string) {
	t.Helper()
	if text == "" {
		if node != nil {
			t.Fatalf("expected nil node, got kind %v", node.Kind)
		}
		return
	}
	if node == nil {
		t.Fatalf("expected %q node, got nil", text)
	}
	if node.Kind != kind {
		t.Fatalf("%q node kind = %v, want %v", text, node.Kind, kind)
	}
	if got := scanner.GetSourceTextOfNodeFromSourceFile(sf, node, false); got != text {
		t.Fatalf("node text = %q, want %q", got, text)
	}
}

func TestExtendsSchemaRequestsHaveNoBrand(t *testing.T) {
	t.Parallel()
	source := `
import { Schema } from "effect"
import { Class } from "effect/Schema"
declare module "effect/Schema" { export { Class as RequestClass } }
class AliasedRequestClass extends Schema.RequestClass<AliasedRequestClass>("request-key")({ value: Schema.String }) {}
class AliasedRequestClassSecondGeneric extends Schema.RequestClass<AliasedRequestClassSecondGeneric, "Brand">("request-key")({ value: Schema.String }) {}
class Unbranded extends Schema.TaggedRequest<Unbranded>("request-key")("request-tag", {
  payload: {}, success: Schema.Void, failure: Schema.Never
}) {}
class SecondGeneric extends Schema.TaggedRequest<SecondGeneric, "Brand">("request-key")("request-tag", {
  payload: {}, success: Schema.Void, failure: Schema.Never
}) {}
`
	c, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectV3Internal(t, source)
	t.Cleanup(done)
	for _, name := range []string{"AliasedRequestClass", "AliasedRequestClassSecondGeneric"} {
		className, _ := findClassTypeByName(t, c, sf, name)
		class := className.Parent
		got := tp.ExtendsSchemaRequestClass(class)
		if got == nil {
			t.Fatal("expected RequestClass match")
		}
		assertSchemaExtendsNode(t, sf, got.ClassName, ast.KindIdentifier, name)
		assertSchemaExtendsNode(t, sf, got.SelfTypeNode, ast.KindTypeReference, name)
		assertSchemaExtendsNode(t, sf, got.BrandTypeNode, ast.KindUnknown, "")
		if tp.ExtendsSchemaRequestClass(class) != got {
			t.Fatal("RequestClass did not return its cached result pointer")
		}
	}
	for _, name := range []string{"Unbranded", "SecondGeneric"} {
		className, _ := findClassTypeByName(t, c, sf, name)
		class := className.Parent
		got := tp.ExtendsSchemaTaggedRequest(class)
		if got == nil {
			t.Fatal("expected TaggedRequest match")
		}
		assertSchemaExtendsNode(t, sf, got.ClassName, ast.KindIdentifier, name)
		assertSchemaExtendsNode(t, sf, got.SelfTypeNode, ast.KindTypeReference, name)
		assertSchemaExtendsNode(t, sf, got.BrandTypeNode, ast.KindUnknown, "")
		assertSchemaExtendsNode(t, sf, got.KeyStringLiteral, ast.KindStringLiteral, `"request-key"`)
		assertSchemaExtendsNode(t, sf, got.TagStringLiteral, ast.KindStringLiteral, `"request-tag"`)
		if tp.ExtendsSchemaTaggedRequest(class) != got {
			t.Fatal("TaggedRequest did not return its cached result pointer")
		}
		if tp.ExtendsSchemaTaggedClass(class) != nil || tp.ExtendsSchemaTaggedError(class) != nil || tp.ExtendsSchemaRequestClass(class) != nil {
			t.Fatal("TaggedRequest matched a different Schema member")
		}
	}
}
