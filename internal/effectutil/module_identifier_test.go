package effectutil

import (
	"testing"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/core"
	"github.com/microsoft/typescript-go/shim/parser"
)

func parseSource(source string) *ast.SourceFile {
	return parser.ParseSourceFile(ast.SourceFileParseOptions{
		FileName: "/test.ts",
	}, source, core.ScriptKindTS)
}

func TestFindModuleIdentifierForPackage_NamespaceImport(t *testing.T) {
	t.Parallel()
	source := `import * as Model from "effect/unstable/schema"`
	sf := parseSource(source)

	got := FindModuleIdentifierForPackage(sf, "effect/unstable", "schema")
	if got != "Model" {
		t.Errorf("FindModuleIdentifierForPackage = %q, want %q", got, "Model")
	}
}

func TestFindModuleIdentifierForPackage_NamedImportWithAlias(t *testing.T) {
	t.Parallel()
	source := `import { Model as M } from "effect/unstable"`
	sf := parseSource(source)

	got := FindModuleIdentifierForPackage(sf, "effect/unstable", "Model")
	if got != "M" {
		t.Errorf("FindModuleIdentifierForPackage = %q, want %q", got, "M")
	}
}

func TestFindModuleIdentifierForPackage_NamedImportNoAlias(t *testing.T) {
	t.Parallel()
	source := `import { Model } from "effect/unstable"`
	sf := parseSource(source)

	got := FindModuleIdentifierForPackage(sf, "effect/unstable", "Model")
	if got != "Model" {
		t.Errorf("FindModuleIdentifierForPackage = %q, want %q", got, "Model")
	}
}

func TestFindModuleIdentifierForPackage_NoMatchingImport(t *testing.T) {
	t.Parallel()
	source := `import { Schema } from "effect"`
	sf := parseSource(source)

	got := FindModuleIdentifierForPackage(sf, "effect/unstable", "Model")
	if got != "Model" {
		t.Errorf("FindModuleIdentifierForPackage = %q, want fallback %q", got, "Model")
	}
}

func TestFindModuleIdentifierForPackage_NilSourceFile(t *testing.T) {
	t.Parallel()
	got := FindModuleIdentifierForPackage(nil, "effect/unstable", "Model")
	if got != "Model" {
		t.Errorf("FindModuleIdentifierForPackage = %q, want fallback %q", got, "Model")
	}
}

func TestFindModuleIdentifier_ExistingBehavior(t *testing.T) {
	t.Parallel()

	t.Run("namespace import", func(t *testing.T) {
		t.Parallel()
		source := `import * as S from "effect/Schema"`
		sf := parseSource(source)
		got := FindModuleIdentifier(sf, "Schema")
		if got != "S" {
			t.Errorf("FindModuleIdentifier = %q, want %q", got, "S")
		}
	})

	t.Run("named import with alias", func(t *testing.T) {
		t.Parallel()
		source := `import { Schema as Sc } from "effect"`
		sf := parseSource(source)
		got := FindModuleIdentifier(sf, "Schema")
		if got != "Sc" {
			t.Errorf("FindModuleIdentifier = %q, want %q", got, "Sc")
		}
	})

	t.Run("named import no alias", func(t *testing.T) {
		t.Parallel()
		source := `import { Schema } from "effect"`
		sf := parseSource(source)
		got := FindModuleIdentifier(sf, "Schema")
		if got != "Schema" {
			t.Errorf("FindModuleIdentifier = %q, want %q", got, "Schema")
		}
	})

	t.Run("fallback", func(t *testing.T) {
		t.Parallel()
		source := `const x = 1`
		sf := parseSource(source)
		got := FindModuleIdentifier(sf, "Schema")
		if got != "Schema" {
			t.Errorf("FindModuleIdentifier = %q, want fallback %q", got, "Schema")
		}
	})
}
