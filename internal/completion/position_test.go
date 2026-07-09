package completion

import (
	"testing"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/core"
	"github.com/microsoft/typescript-go/shim/parser"
	"github.com/microsoft/typescript-go/shim/scanner"
)

func parseSource(source string) *ast.SourceFile {
	return parser.ParseSourceFile(ast.SourceFileParseOptions{
		FileName: "/test.ts",
	}, source, core.ScriptKindTS)
}

func TestParseDataForExtendsClassCompletion_AfterDot(t *testing.T) {
	t.Parallel()
	// class Foo extends Schema.| (cursor after the dot)
	source := `class Foo extends Schema.`
	sf := parseSource(source)
	pos := len(source)

	data := ParseDataForExtendsClassCompletion(sf, pos)
	if data == nil {
		t.Fatal("expected non-nil result for 'class Foo extends Schema.'")
	}
	if got := data.AccessedObjectText(); got != "Schema" {
		t.Errorf("AccessedObjectText = %q, want %q", got, "Schema")
	}
	if got := data.ClassNameText(); got != "Foo" {
		t.Errorf("ClassNameText = %q, want %q", got, "Foo")
	}
}

func TestParseDataForExtendsClassCompletion_AfterPropertyName(t *testing.T) {
	t.Parallel()
	// class Foo extends Schema.Class| (cursor after Class)
	source := `class Foo extends Schema.Class`
	sf := parseSource(source)
	pos := len(source)

	data := ParseDataForExtendsClassCompletion(sf, pos)
	if data == nil {
		t.Fatal("expected non-nil result for 'class Foo extends Schema.Class'")
	}
	if got := data.AccessedObjectText(); got != "Schema" {
		t.Errorf("AccessedObjectText = %q, want %q", got, "Schema")
	}
	if got := data.ClassNameText(); got != "Foo" {
		t.Errorf("ClassNameText = %q, want %q", got, "Foo")
	}
}

func TestParseDataForExtendsClassCompletion_StandaloneIdentifier(t *testing.T) {
	t.Parallel()
	// class Foo extends Schema| (cursor after standalone identifier)
	source := `class Foo extends Schema`
	sf := parseSource(source)
	pos := len(source)

	data := ParseDataForExtendsClassCompletion(sf, pos)
	if data == nil {
		t.Fatal("expected non-nil result for 'class Foo extends Schema'")
	} else {
		if got := scanner.GetTextOfNode(data.AccessedObject); got != "Schema" {
			t.Errorf("AccessedObject text = %q, want %q", got, "Schema")
		}
		if got := data.ClassNameText(); got != "Foo" {
			t.Errorf("ClassNameText = %q, want %q", got, "Foo")
		}
	}
}

func TestParseDataForExtendsClassCompletion_NonClassContext(t *testing.T) {
	t.Parallel()
	// Not in a class extends clause
	source := `const x = Schema.Class`
	sf := parseSource(source)
	pos := len(source)

	data := ParseDataForExtendsClassCompletion(sf, pos)
	if data != nil {
		t.Error("expected nil result for non-class context")
	}
}

func TestParseDataForExtendsClassCompletion_InsideImportDeclaration(t *testing.T) {
	t.Parallel()
	source := `import { Schema } from "effect"`
	sf := parseSource(source)
	// Position after "Schema" inside the import
	pos := len(`import { Schema`)

	data := ParseDataForExtendsClassCompletion(sf, pos)
	if data != nil {
		t.Error("expected nil result for position inside import declaration")
	}
}

func TestParseDataForExtendsClassCompletion_ReplacementSpan(t *testing.T) {
	t.Parallel()
	source := `class Foo extends Schema.Class`
	sf := parseSource(source)
	pos := len(source)

	data := ParseDataForExtendsClassCompletion(sf, pos)
	if data == nil {
		t.Fatal("expected non-nil result")
	} else {
		// The replacement span should cover "Schema.Class"
		if data.ReplacementLength <= 0 {
			t.Errorf("expected positive ReplacementLength, got %d", data.ReplacementLength)
		}
		replaced := source[data.ReplacementStart : data.ReplacementStart+data.ReplacementLength]
		if replaced != "Schema.Class" {
			t.Errorf("replacement span covers %q, want %q", replaced, "Schema.Class")
		}
	}
}
