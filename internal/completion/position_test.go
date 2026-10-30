package completion

import (
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

func TestParseDataForExtendsClassCompletion(t *testing.T) {
	t.Parallel()
	for _, tc := range []struct {
		source       string
		wantObject   string
		wantReplaced string
	}{
		{`class Foo extends Schema.`, "Schema", "Schema."},
		{`class Foo extends X.Schema.`, "X.Schema", "X.Schema."},
		{`class Foo extends Schema.Class`, "Schema", "Schema.Class"},
		{`class Foo extends Schema`, "Schema", "Schema"},
	} {
		t.Run(tc.source, func(t *testing.T) {
			t.Parallel()
			data := ParseDataForExtendsClassCompletion(parseSource(tc.source), len(tc.source))
			if data == nil {
				t.Fatalf("expected non-nil result for %q", tc.source)
			}
			if got := data.AccessedObjectText(); got != tc.wantObject {
				t.Errorf("AccessedObjectText = %q, want %q", got, tc.wantObject)
			}
			if got := data.ClassNameText(); got != "Foo" {
				t.Errorf("ClassNameText = %q, want %q", got, "Foo")
			}
			if got := tc.source[data.ReplacementStart : data.ReplacementStart+data.ReplacementLength]; got != tc.wantReplaced {
				t.Errorf("replacement span covers %q, want %q", got, tc.wantReplaced)
			}
		})
	}
}

func TestParseDataForExtendsClassCompletion_Rejected(t *testing.T) {
	t.Parallel()
	for _, tc := range []struct {
		name     string
		source   string
		position int
	}{
		{"non-class context", `const x = Schema.Class`, len(`const x = Schema.Class`)},
		{"inside import declaration", `import { Schema } from "effect"`, len(`import { Schema`)},
	} {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()
			if data := ParseDataForExtendsClassCompletion(parseSource(tc.source), tc.position); data != nil {
				t.Errorf("expected nil result for %q at %d", tc.source, tc.position)
			}
		})
	}
}
