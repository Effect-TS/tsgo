package refactors

import (
	"fmt"

	"github.com/effect-ts/tsgo/internal/refactor"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/astnav"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/effect-ts/tsgo/internal/rewriter"
)

var AsyncAwaitToFnTryPromise = refactor.Refactor{
	Name:        "asyncAwaitToFnTryPromise",
	Description: "Convert async/await to Effect.fn (with error handling)",
	Kind:        "rewrite.effect.asyncAwaitToFnTryPromise",
	Run:         runAsyncAwaitToFnTryPromise,
}

func runAsyncAwaitToFnTryPromise(ctx *refactor.Context) []ls.CodeAction {
	token := astnav.GetTokenAtPosition(ctx.SourceFile, ctx.Span.Pos())
	if token == nil {
		return nil
	}

	// Walk ancestor chain to find the nearest async function
	asyncFn := findAsyncFunction(token)
	if asyncFn == nil {
		return nil
	}

	effectModuleName := typeparser.FindEffectModuleIdentifier(ctx.SourceFile)
	dataModuleName := typeparser.FindModuleIdentifier(ctx.SourceFile, "Data")

	action := ctx.NewRefactorAction(refactor.RefactorAction{
		Description: "Rewrite to Effect.fn with failures",
		Run: func(tracker *rewriter.Tracker) {
			transformAsyncToEffectFnTryPromise(tracker, ctx.SourceFile, asyncFn, effectModuleName, dataModuleName)
		},
	})
	if action == nil {
		return nil
	}

	action.Kind = "refactor.rewrite.effect.asyncAwaitToFnTryPromise"
	return []ls.CodeAction{*action}
}

func transformAsyncToEffectFnTryPromise(tracker *rewriter.Tracker, sf *ast.SourceFile, node *ast.Node, effectModuleName string, dataModuleName string) {
	body := typeparser.GetFunctionLikeBody(node)
	if body == nil {
		return
	}

	// Track error classes to generate
	errorCount := 0
	var errorClasses []*ast.Node

	// Determine if function is exported (error classes should match)
	isExported := ast.GetCombinedModifierFlags(node)&ast.ModifierFlagsExport != 0
	// Also check parent variable statement for export
	if !isExported {
		varStmt := findParentOfKind(node, ast.KindVariableStatement)
		if varStmt != nil {
			isExported = ast.GetCombinedModifierFlags(varStmt)&ast.ModifierFlagsExport != 0
		}
	}

	// Transform await expressions to yield* Effect.tryPromise(...)
	transformedBody := transformBodyAwaitToYield(tracker, body, func(t *rewriter.Tracker, expr *ast.Node) *ast.Node {
		errorCount++
		errorName := fmt.Sprintf("Error%d", errorCount)

		// Build the error class declaration
		errorClass := buildTaggedErrorClass(t, dataModuleName, errorName, isExported)
		errorClasses = append(errorClasses, errorClass)

		return buildYieldStarTryPromise(t, expr, effectModuleName, errorName)
	})

	// Get the function name for the trace string
	fnName := typeparser.GetFunctionLikeName(node)

	// Build Effect.fn("name")(function*(params) { body })
	effectFnCall := buildEffectFnCall(tracker, node, transformedBody, effectModuleName, fnName)

	// Build declaration wrapping the fn call
	newDecl := buildFnDeclaration(tracker, node, effectFnCall, fnName)
	if newDecl == nil {
		return
	}

	ast.SetParentInChildren(newDecl)

	// Find the top-level statement containing the function (for inserting error classes before it)
	topLevelStmt := findTopLevelStatement(node)

	// Insert error classes before the top-level statement
	for _, errorClass := range errorClasses {
		ast.SetParentInChildren(errorClass)
		tracker.InsertNodeBefore(sf, topLevelStmt, errorClass, true, rewriter.LeadingTriviaOptionNone)
	}

	// Replace the function
	if node.Kind == ast.KindFunctionDeclaration {
		tracker.ReplaceNode(sf, node, newDecl, nil)
	} else {
		replaceExpressionViaStatement(tracker, sf, node, newDecl)
	}
}
