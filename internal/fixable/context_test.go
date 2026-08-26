package fixable

import (
	"testing"

	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	"github.com/microsoft/TypeScript/tsc/shim/lsp/lsproto"
	"github.com/microsoft/TypeScript/tsc/shim/parser"
)

func TestStandaloneRangeConversionUsesLSPLineSemantics(t *testing.T) {
	t.Parallel()

	sourceFile := parser.ParseSourceFile(ast.SourceFileParseOptions{
		FileName: "/source.ts",
		Path:     "/source.ts",
	}, "😀x\u2028y", core.ScriptKindTS)
	ctx := &Context{
		SourceFile: sourceFile,
		converters: newStandaloneConverters(sourceFile),
	}

	for _, test := range []struct {
		name  string
		start uint32
		end   uint32
		want  string
	}{
		{name: "after non-BMP character", start: 2, end: 3, want: "x"},
		{name: "after Unicode line separator", start: 4, end: 5, want: "y"},
	} {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			rng := ctx.LSPRangeToTextRange(lsproto.Range{
				Start: lsproto.Position{Line: 0, Character: test.start},
				End:   lsproto.Position{Line: 0, Character: test.end},
			})
			if got := sourceFile.Text()[rng.Pos():rng.End()]; got != test.want {
				t.Fatalf("unexpected converted range text: %q", got)
			}
			if position := ctx.BytePosToLSPPosition(rng.Pos()); position.Line != 0 || position.Character != test.start {
				t.Fatalf("unexpected round-trip position: %+v", position)
			}
		})
	}
}
