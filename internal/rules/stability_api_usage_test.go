package rules

import (
	"testing"

	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	"github.com/microsoft/TypeScript/tsc/shim/parser"
)

func TestStabilityOfDeclaration(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name   string
		source string
		want   string
	}{
		{"unstable variable", "/** @stability unstable */\nexport const api = 1", "unstable"},
		{"experimental variable", "/** @stability experimental */\nexport const api = 1", "experimental"},
		{"stable variable", "/** @stability stable */\nexport const api = 1", ""},
		{"other tag", "/** @deprecated */\nexport const api = 1", ""},
		{"plain variable", "export const api = 1", ""},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			sf := parser.ParseSourceFile(ast.SourceFileParseOptions{FileName: "/test.ts"}, test.source, core.ScriptKindTS)
			statement := sf.Statements.Nodes[0]
			declaration := statement.AsVariableStatement().DeclarationList.AsVariableDeclarationList().Declarations.Nodes[0]
			if got := stabilityOfDeclaration(declaration); got != test.want {
				t.Errorf("stabilityOfDeclaration = %q, want %q", got, test.want)
			}
		})
	}
}
