package typeparser

import (
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
)

func TestExtendsDataError(t *testing.T) {
	t.Parallel()
	if err := bundledeffect.EnsurePackageInstalled(bundledeffect.EffectV3, "effect"); err != nil {
		t.Skip("Effect v3 not installed:", err)
	}
	_, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectV3Internal(t, `
import { Data } from "effect"
interface Payload { name: string }
class Inline extends Data.Error<{ name: string }> {}
class Parenthesized extends (Data).Error<{ message: string }> {}
class NonNull extends Data!.Error<{ message: string }> {}
function getData() { return Data }
class Returned extends getData().Error<{ message: string }> {}
class Referenced extends Data.Error<Payload> {}
class Empty extends Data.Error {}
class Tagged extends Data.TaggedError("Tagged")<{ name: string }> {}
const UnrelatedData = { Error: class<Fields> { declare fields: Fields } }
function shadowedData() {
  const Data = UnrelatedData
  class Shadowed extends Data.Error<{ name: string }> {}
}
const EffectData = Data
function aliasedData() {
  const Data = EffectData
  class Aliased extends Data.Error<{ name: string }> {}
}
`)
	defer done()
	want := map[string]ast.Kind{
		"Inline":        ast.KindTypeLiteral,
		"Parenthesized": ast.KindTypeLiteral,
		"NonNull":       ast.KindTypeLiteral,
		"Returned":      ast.KindTypeLiteral,
		"Referenced":    ast.KindTypeReference,
		"Empty":         ast.KindUnknown,
		"Aliased":       ast.KindTypeLiteral,
	}
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node.Kind == ast.KindClassDeclaration {
			name := node.Name().Text()
			result := tp.ExtendsDataError(node)
			if name == "Tagged" || name == "Shadowed" {
				if result != nil {
					t.Fatalf("class %s must not match Data.Error", name)
				}
			} else {
				if result == nil || result.ClassName().Text() != name {
					t.Fatalf("expected Data.Error class %s", name)
				}
				fields := result.FieldsTypeNode()
				if name == "Empty" {
					if fields != nil {
						t.Fatal("expected absent fields to remain nil")
					}
				} else if fields == nil || fields.Kind != want[name] {
					t.Fatalf("expected %s payload kind %s", name, want[name])
				}
				if tp.ExtendsDataTaggedError(node) != nil {
					t.Fatalf("Data.Error class %s must not match Data.TaggedError", name)
				}
				delete(want, name)
			}
		}
		node.ForEachChild(walk)
		return false
	}
	walk(sf.AsNode())
	if len(want) != 0 {
		t.Fatalf("missing classes: %v", want)
	}
}
