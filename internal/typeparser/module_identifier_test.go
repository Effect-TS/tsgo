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

func TestFindModuleReferenceForPackage_NamespaceImport(t *testing.T) {
	t.Parallel()
	source := `import * as SchemaModules from "effect/schema"`
	sf := parseSource(source)

	got := FindModuleReferenceForPackage(sf, "effect", "schema").String()
	if got != "SchemaModules" {
		t.Errorf("FindModuleReferenceForPackage = %q, want %q", got, "SchemaModules")
	}
}

func TestFindModuleReferenceForPackage_NamedImportWithAlias(t *testing.T) {
	t.Parallel()
	source := `import { Model as M } from "effect/schema"`
	sf := parseSource(source)

	got := FindModuleReferenceForPackage(sf, "effect/schema", "Model").String()
	if got != "M" {
		t.Errorf("FindModuleReferenceForPackage = %q, want %q", got, "M")
	}
}

func TestFindModuleReferenceForPackage_NamedImportNoAlias(t *testing.T) {
	t.Parallel()
	source := `import { Model } from "effect/schema"`
	sf := parseSource(source)

	got := FindModuleReferenceForPackage(sf, "effect/schema", "Model").String()
	if got != "Model" {
		t.Errorf("FindModuleReferenceForPackage = %q, want %q", got, "Model")
	}
}

func TestFindModuleReferenceForPackage_NoMatchingImport(t *testing.T) {
	t.Parallel()
	source := `import { Schema } from "effect"`
	sf := parseSource(source)

	got := FindModuleReferenceForPackage(sf, "effect/schema", "Model").String()
	if got != "Model" {
		t.Errorf("FindModuleReferenceForPackage = %q, want fallback %q", got, "Model")
	}
}

func TestFindModuleReferenceForPackage_NilSourceFile(t *testing.T) {
	t.Parallel()
	got := FindModuleReferenceForPackage(nil, "effect/schema", "Model").String()
	if got != "Model" {
		t.Errorf("FindModuleReferenceForPackage = %q, want fallback %q", got, "Model")
	}
}

func TestFindModuleReference_ExistingBehavior(t *testing.T) {
	t.Parallel()

	t.Run("namespace import", func(t *testing.T) {
		t.Parallel()
		source := `import * as S from "effect/Schema"`
		sf := parseSource(source)
		got := FindModuleReference(sf, "Schema").String()
		if got != "S" {
			t.Errorf("FindModuleReference = %q, want %q", got, "S")
		}
	})

	t.Run("named import with alias", func(t *testing.T) {
		t.Parallel()
		source := `import { Schema as Sc } from "effect"`
		sf := parseSource(source)
		got := FindModuleReference(sf, "Schema").String()
		if got != "Sc" {
			t.Errorf("FindModuleReference = %q, want %q", got, "Sc")
		}
	})

	t.Run("named import no alias", func(t *testing.T) {
		t.Parallel()
		source := `import { Schema } from "effect"`
		sf := parseSource(source)
		got := FindModuleReference(sf, "Schema").String()
		if got != "Schema" {
			t.Errorf("FindModuleReference = %q, want %q", got, "Schema")
		}
	})

	t.Run("fallback", func(t *testing.T) {
		t.Parallel()
		source := `const x = 1`
		sf := parseSource(source)
		got := FindModuleReference(sf, "Schema").String()
		if got != "Schema" {
			t.Errorf("FindModuleReference = %q, want fallback %q", got, "Schema")
		}
	})
}

func TestFindModuleReference_BarrelNamespaceImport(t *testing.T) {
	t.Parallel()
	cases := []struct {
		name, source, pkg, module, want string
	}{
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
