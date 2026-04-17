package refactors

import (
	"github.com/effect-ts/tsgo/internal/refactor"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/astnav"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
)

var WrapWithEffectGen = refactor.Refactor{
	Name:        "wrapWithEffectGen",
	Description: "Wrap with Effect.gen",
	Kind:        "rewrite.effect.wrapWithEffectGen",
	Run:         runWrapWithEffectGen,
}

func runWrapWithEffectGen(ctx *refactor.Context) []ls.CodeAction {
	token := astnav.GetTokenAtPosition(ctx.SourceFile, ctx.Span.Pos())
	if token == nil {
		return nil
	}

	// Walk ancestor chain looking for an expression whose type is a strict Effect type
	var matchedNode *ast.Node
	for node := token; node != nil; node = node.Parent {
		if !ast.IsExpression(node) {
			continue
		}
		// Skip nodes inside heritage clauses (class extends/implements)
		if isInHeritageClause(node) {
			continue
		}
		// Skip if this is the LHS of a variable declaration (not the initializer)
		if node.Parent != nil && node.Parent.Kind == ast.KindVariableDeclaration {
			varDecl := node.Parent.AsVariableDeclaration()
			if varDecl.Initializer != node {
				continue
			}
		}

		nodeType := ctx.TypeParser.GetTypeAtLocation(node)
		if nodeType == nil {
			continue
		}
		if !ctx.TypeParser.StrictIsEffectType(nodeType, node) {
			continue
		}
		if ctx.TypeParser.EffectGenCall(node) != nil {
			continue
		}

		matchedNode = node
		break
	}

	if matchedNode == nil {
		return nil
	}

	effectModuleName := typeparser.FindEffectModuleIdentifier(ctx.SourceFile)

	action := ctx.NewRefactorAction(refactor.RefactorAction{
		Description: "Wrap with Effect.gen",
		Run: func(tracker *rewriter.Tracker) {
			start := astnav.GetStartOfNode(matchedNode, ctx.SourceFile, false)
			textRange := ctx.SourceFile.Text()[start:matchedNode.End()]
			wrapped := effectModuleName + ".gen(function*() { return yield* " + textRange + " })"
			tracker.ReplaceRangeWithText(ctx.SourceFile, lsproto.Range{
				Start: ctx.BytePosToLSPPosition(start),
				End:   ctx.BytePosToLSPPosition(matchedNode.End()),
			}, wrapped)
		},
	})
	if action == nil {
		return nil
	}

	action.Kind = "refactor.rewrite.effect.wrapWithEffectGen"
	return []ls.CodeAction{*action}
}

// isInHeritageClause checks if a node is inside a heritage clause (extends/implements).
func isInHeritageClause(node *ast.Node) bool {
	for parent := node.Parent; parent != nil; parent = parent.Parent {
		if parent.Kind == ast.KindHeritageClause {
			return true
		}
		// Stop walking once we hit a statement or declaration boundary
		if parent.Kind == ast.KindClassDeclaration || parent.Kind == ast.KindSourceFile {
			return false
		}
	}
	return false
}
