package rewriter_test

import (
	"context"
	"testing"

	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	"github.com/microsoft/TypeScript/tsc/shim/ls/change"
	"github.com/microsoft/TypeScript/tsc/shim/ls/lsconv"
	"github.com/microsoft/TypeScript/tsc/shim/ls/lsutil"
	"github.com/microsoft/TypeScript/tsc/shim/lsp/lsproto"
	"github.com/microsoft/TypeScript/tsc/shim/parser"
	"github.com/microsoft/TypeScript/tsc/shim/tspath"
)

func TestNewPaths(t *testing.T) {
	t.Parallel()
	for _, test := range []struct {
		name     string
		names    []string
		typeName bool
		expected string
	}{
		{name: "single expression", names: []string{"Effect"}, expected: "const value: target = Effect"},
		{name: "expression path", names: []string{"X", "Effect", "map"}, expected: "const value: target = X.Effect.map"},
		{name: "single type", names: []string{"Schema"}, typeName: true, expected: "const value: Schema = target"},
		{name: "type path", names: []string{"X", "Schema", "Schema"}, typeName: true, expected: "const value: X.Schema.Schema = target"},
	} {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			source := "const value: target = target"
			sf := parser.ParseSourceFile(ast.SourceFileParseOptions{FileName: "/test.ts"}, source, core.ScriptKindTS)
			declaration := sf.Statements.Nodes[0].AsVariableStatement().DeclarationList.AsVariableDeclarationList().Declarations.Nodes[0].AsVariableDeclaration()
			converters := lsconv.NewConverters(lsproto.PositionEncodingKindUTF16, func(tspath.RootedFilePath) *lsconv.LSPLineMap {
				return lsconv.ComputeLSPLineStarts(source)
			})
			tracker := rewriter.NewTracker(change.NewTracker(context.Background(), &core.CompilerOptions{}, lsutil.GetDefaultFormatCodeSettings(), converters))
			if test.typeName {
				node := tracker.NewQualifiedNamePath(test.names...)
				if len(test.names) > 1 && node.Kind != ast.KindQualifiedName {
					t.Fatalf("kind = %v, want QualifiedName", node.Kind)
				}
				tracker.ReplaceNode(sf, declaration.Type.AsTypeReferenceNode().TypeName, node, nil)
			} else {
				node := tracker.NewPropertyAccessPath(test.names...)
				if len(test.names) > 1 && node.Kind != ast.KindPropertyAccessExpression {
					t.Fatalf("kind = %v, want PropertyAccessExpression", node.Kind)
				}
				tracker.ReplaceNode(sf, declaration.Initializer, node, nil)
			}
			var changes []core.TextChange
			for _, edit := range tracker.GetChanges()[sf.FileName()] {
				changes = append(changes, core.TextChange{TextRange: lsconv.FromLSPRangeToOriginal(converters, sf, edit.Range), NewText: edit.NewText})
			}
			if got := core.ApplyBulkEdits(source, changes); got != test.expected {
				t.Fatalf("replacement = %q, want %q", got, test.expected)
			}
		})
	}
	tracker := rewriter.NewTracker(change.NewTracker(context.Background(), &core.CompilerOptions{}, lsutil.GetDefaultFormatCodeSettings(), nil))
	if tracker.NewPropertyAccessPath() != nil || tracker.NewQualifiedNamePath() != nil {
		t.Fatal("empty paths must build no node")
	}
}
