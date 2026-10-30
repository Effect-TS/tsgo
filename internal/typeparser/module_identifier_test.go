package typeparser

import (
	"slices"
	"testing"

	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	"github.com/microsoft/TypeScript/tsc/shim/parser"
)

func parseSource(source string) *ast.SourceFile {
	return parser.ParseSourceFile(ast.SourceFileParseOptions{
		FileName: "/test.ts",
	}, source, core.ScriptKindTS)
}

func TestFindModuleReferenceForPackage(t *testing.T) {
	t.Parallel()
	cases := []struct {
		name, source, pkg, module, want string
	}{
		{"namespace import", `import * as SchemaModules from "effect/schema"`, "effect", "schema", "SchemaModules"},
		{"named import with alias", `import { Model as M } from "effect/schema"`, "effect/schema", "Model", "M"},
		{"named import no alias", `import { Model } from "effect/schema"`, "effect/schema", "Model", "Model"},
		{"no matching import", `import { Schema } from "effect"`, "effect/schema", "Model", "Model"},
		{"effect barrel", `import * as X from "effect"`, "effect", "Effect", "X.Effect"},
		{"effect barrel other module", `import * as X from "effect"`, "effect", "Layer", "X.Layer"},
		{"schema barrel", `import * as S from "effect/schema"`, "effect/schema", "Model", "S.Model"},
		{"direct import wins over barrel", "import * as X from \"effect\"\nimport { Effect as E } from \"effect\"", "effect", "Effect", "E"},
		{"module namespace wins over barrel", "import * as X from \"effect\"\nimport * as Eff from \"effect/Effect\"", "effect", "Effect", "Eff"},
		{"other package barrel ignored", `import * as X from "effectful"`, "effect", "Effect", "Effect"},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			t.Parallel()
			got := FindModuleReferenceForPackage(parseSource(c.source), c.pkg, c.module)
			if got.String() != c.want {
				t.Errorf("FindModuleReferenceForPackage = %q, want %q", got.String(), c.want)
			}
		})
	}
	if got := FindModuleReferenceForPackage(nil, "effect/schema", "Model").String(); got != "Model" {
		t.Errorf("FindModuleReferenceForPackage(nil) = %q, want fallback %q", got, "Model")
	}
}

func TestFindModuleReference(t *testing.T) {
	t.Parallel()
	cases := []struct {
		name, source, want string
	}{
		{"namespace import", `import * as S from "effect/Schema"`, "S"},
		{"named import with alias", `import { Schema as Sc } from "effect"`, "Sc"},
		{"named import no alias", `import { Schema } from "effect"`, "Schema"},
		{"fallback", `const x = 1`, "Schema"},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			t.Parallel()
			if got := FindModuleReference(parseSource(c.source), "Schema").String(); got != c.want {
				t.Errorf("FindModuleReference = %q, want %q", got, c.want)
			}
		})
	}
}

func TestFindEffectModuleReference(t *testing.T) {
	t.Parallel()
	cases := []struct {
		name, source string
		want         ModuleReference
	}{
		{"named import", `import { Effect } from "effect"`, ModuleReference{"Effect"}},
		{"named import with alias", `import { Effect as E } from "effect"`, ModuleReference{"E"}},
		{"module namespace import", `import * as T from "effect/Effect"`, ModuleReference{"T"}},
		{"barrel namespace import", `import * as X from "effect"`, ModuleReference{"X", "Effect"}},
		{"named function import falls back", `import { flatMap } from "effect/Effect"`, ModuleReference{"Effect"}},
		{"fallback", `const x = 1`, ModuleReference{"Effect"}},
		{"type-only named import skipped", "import type { Effect } from \"effect\"\nimport * as X from \"effect\"", ModuleReference{"X", "Effect"}},
		{"type-only specifier skipped", `import { type Effect, Layer } from "effect"`, ModuleReference{"Effect"}},
		{"type-only namespace import skipped", `import type * as T from "effect/Effect"`, ModuleReference{"Effect"}},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			t.Parallel()
			got := FindEffectModuleReference(parseSource(c.source))
			if !slices.Equal(got, c.want) {
				t.Errorf("FindEffectModuleReference = %q, want %q", got, c.want)
			}
		})
	}
	if got := FindEffectModuleReference(nil); !slices.Equal(got, ModuleReference{"Effect"}) {
		t.Errorf("FindEffectModuleReference(nil) = %q, want fallback", got)
	}
}

func TestModuleReferenceMatchesNode(t *testing.T) {
	t.Parallel()
	sf := parseSource("X.Effect\nEffect\nY.Effect\nX.Effect.map\nlet a: X.Effect")
	expression := func(i int) *ast.Node { return sf.Statements.Nodes[i].AsExpressionStatement().Expression }
	ref := ModuleReference{"X", "Effect"}
	if !ref.MatchesNode(expression(0)) {
		t.Error("X.Effect should match")
	}
	if ref.MatchesNode(expression(1)) || ref.MatchesNode(expression(2)) || ref.MatchesNode(expression(3)) {
		t.Error("Effect, Y.Effect and X.Effect.map should not match")
	}
	if !(ModuleReference{"Effect"}).MatchesNode(expression(1)) {
		t.Error("Effect should match")
	}
	typeName := sf.Statements.Nodes[4].AsVariableStatement().DeclarationList.AsVariableDeclarationList().Declarations.Nodes[0].AsVariableDeclaration().Type.AsTypeReferenceNode().TypeName
	if !ref.MatchesNode(typeName) {
		t.Error("qualified name X.Effect should match")
	}
}
