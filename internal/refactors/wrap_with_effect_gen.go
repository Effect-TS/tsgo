package refactors

import (
	"github.com/effect-ts/effect-typescript-go/internal/effectutil"
	"github.com/effect-ts/effect-typescript-go/internal/refactor"
	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/astnav"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
)

var WrapWithEffectGen = refactor.Refactor{
	Name:        "wrapWithEffectGen",
	Description: "Wrap with Effect.gen",
	Kind:        "rewrite.effect.wrapWithEffectGen",
	Run:         runWrapWithEffectGen,
}

func runWrapWithEffectGen(ctx *refactor.Context) []ls.CodeAction {
	// Only applicable with a non-empty selection
	if ctx.Span.Pos() == ctx.Span.End() {
		return nil
	}

	c, done := ctx.GetTypeCheckerForFile(ctx.SourceFile)
	if c == nil {
		return nil
	}
	defer done()

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

		nodeType := c.GetTypeAtLocation(node)
		if nodeType == nil {
			continue
		}
		if !typeparser.StrictIsEffectType(c, nodeType, node) {
			continue
		}

		matchedNode = node
		break
	}

	if matchedNode == nil {
		return nil
	}

	effectModuleName := effectutil.FindEffectModuleIdentifier(ctx.SourceFile)

	action := ctx.NewRefactorAction(refactor.RefactorAction{
		Description: "Wrap with Effect.gen",
		Run: func(tracker *change.Tracker) {
			// Build: Effect.gen(function*() { return yield* <expr> })
			clonedExpr := tracker.DeepCloneNode(matchedNode)

			// yield* <expr>
			yieldExpr := tracker.NewYieldExpression(
				tracker.NewToken(ast.KindAsteriskToken),
				clonedExpr,
			)

			// return yield* <expr>
			returnStmt := tracker.NewReturnStatement(yieldExpr)

			// { return yield* <expr> }
			body := tracker.NewBlock(
				tracker.NewNodeList([]*ast.Node{returnStmt}),
				false,
			)

			// function*() { return yield* <expr> }
			genFn := tracker.NewFunctionExpression(
				nil,                                    // modifiers
				tracker.NewToken(ast.KindAsteriskToken), // asterisk (generator)
				nil,                                    // name
				nil,                                    // typeParameters
				tracker.NewNodeList([]*ast.Node{}),     // parameters (empty)
				nil,                                    // returnType
				nil,                                    // fullSignature
				body,
			)

			// Effect.gen(...)
			effectId := tracker.NewIdentifier(effectModuleName)
			genAccess := tracker.NewPropertyAccessExpression(
				effectId, nil, tracker.NewIdentifier("gen"), ast.NodeFlagsNone,
			)
			effectGenCall := tracker.NewCallExpression(
				genAccess, nil, nil,
				tracker.NewNodeList([]*ast.Node{genFn}),
				ast.NodeFlagsNone,
			)

			ast.SetParentInChildren(effectGenCall)
			tracker.ReplaceNode(ctx.SourceFile, matchedNode, effectGenCall, nil)
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
