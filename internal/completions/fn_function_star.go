package completions

import (
	"fmt"

	"github.com/effect-ts/tsgo/internal/completion"
	"github.com/effect-ts/tsgo/internal/effectutil"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// fnFunctionStar provides completions for `fn(function*(){})` when dot-accessing
// the Effect module identifier (e.g., `Effect.fn|`). This is AST-based and does
// not require the type checker.
var fnFunctionStar = completion.Completion{
	Name:        "fnFunctionStar",
	Description: "Provides fn(function*(){}) completion when accessing the Effect module",
	Run:         runFnFunctionStar,
}

func runFnFunctionStar(ctx *completion.Context) []*lsproto.CompletionItem {
	result := completion.ParseAccessedExpressionForCompletion(ctx.SourceFile, ctx.Position)
	if result == nil {
		return nil
	}

	// Resolve the local Effect module identifier and compare
	effectIdentifier := effectutil.FindEffectModuleIdentifier(ctx.SourceFile)
	accessedText := scanner.GetTextOfNode(result.AccessedObject)
	if accessedText != effectIdentifier {
		return nil
	}

	// Replacement span: from after the dot to the cursor position
	spanStart := result.AccessedObject.End() + 1
	spanLength := max(ctx.Position-spanStart, 0)

	replacementRange := byteSpanToRange(ctx, spanStart, spanLength)
	sortText := "11"

	var items []*lsproto.CompletionItem

	// Try to find enclosing variable declaration to extract variable name
	varName := findEnclosingVariableName(result.OuterNode)
	if varName != "" {
		items = append(items, makeCompletionItem(
			fmt.Sprintf(`fn("%s")`, varName),
			fmt.Sprintf(`fn("%s")(function*(${1}){${0}})`, varName),
			sortText,
			replacementRange,
		))
	}

	// Always add generic fn and fnUntraced
	items = append(items,
		makeCompletionItem(
			"fn(function*(){})",
			"fn(function*(${1}){${0}})",
			sortText,
			replacementRange,
		),
		makeCompletionItem(
			"fnUntraced(function*(){})",
			"fnUntraced(function*(${1}){${0}})",
			sortText,
			replacementRange,
		),
	)

	return items
}

// findEnclosingVariableName walks up from the given node to find an enclosing
// VariableDeclaration and returns its identifier name, or "" if not found.
func findEnclosingVariableName(node *ast.Node) string {
	varDecl := ast.FindAncestor(node, func(n *ast.Node) bool {
		return n.Kind == ast.KindVariableDeclaration
	})
	if varDecl == nil {
		return ""
	}

	name := varDecl.Name()
	if name == nil || !ast.IsIdentifier(name) {
		return ""
	}

	return scanner.GetTextOfNode(name)
}
