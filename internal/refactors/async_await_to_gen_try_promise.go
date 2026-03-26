package refactors

import (
	"fmt"

	"github.com/effect-ts/tsgo/internal/effectutil"
	"github.com/effect-ts/tsgo/internal/refactor"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/astnav"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
)

var AsyncAwaitToGenTryPromise = refactor.Refactor{
	Name:        "asyncAwaitToGenTryPromise",
	Description: "Convert async/await to Effect.gen (with error handling)",
	Kind:        "rewrite.effect.asyncAwaitToGenTryPromise",
	Run:         runAsyncAwaitToGenTryPromise,
}

func runAsyncAwaitToGenTryPromise(ctx *refactor.Context) []ls.CodeAction {
	token := astnav.GetTokenAtPosition(ctx.SourceFile, ctx.Span.Pos())
	if token == nil {
		return nil
	}

	// Walk ancestor chain to find the nearest async function
	asyncFn := findAsyncFunction(token)
	if asyncFn == nil {
		return nil
	}

	effectModuleName := effectutil.FindEffectModuleIdentifier(ctx.SourceFile)
	dataModuleName := effectutil.FindModuleIdentifier(ctx.SourceFile, "Data")

	action := ctx.NewRefactorAction(refactor.RefactorAction{
		Description: "Rewrite to Effect.gen with failures",
		Run: func(tracker *change.Tracker) {
			transformAsyncToEffectGenTryPromise(tracker, ctx.SourceFile, asyncFn, effectModuleName, dataModuleName)
		},
	})
	if action == nil {
		return nil
	}

	action.Kind = "refactor.rewrite.effect.asyncAwaitToGenTryPromise"
	return []ls.CodeAction{*action}
}

func transformAsyncToEffectGenTryPromise(tracker *change.Tracker, sf *ast.SourceFile, node *ast.Node, effectModuleName string, dataModuleName string) {
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
	transformedBody := transformBodyAwaitToYield(tracker, body, func(t *change.Tracker, expr *ast.Node) *ast.Node {
		errorCount++
		errorName := fmt.Sprintf("Error%d", errorCount)

		// Build the error class declaration
		errorClass := buildTaggedErrorClass(t, dataModuleName, errorName, isExported)
		errorClasses = append(errorClasses, errorClass)

		return buildYieldStarTryPromise(t, expr, effectModuleName, errorName)
	})

	// Build Effect.gen(function*() { transformedBody })
	effectGenCall := buildEffectGenCall(tracker, transformedBody, effectModuleName)

	// Build non-async declaration wrapping the gen call
	newDecl := buildNonAsyncDeclaration(tracker, node, effectGenCall)
	if newDecl == nil {
		return
	}

	ast.SetParentInChildren(newDecl)

	// Find the top-level statement containing the function (for inserting error classes before it)
	topLevelStmt := findTopLevelStatement(node)

	// Insert error classes before the top-level statement
	for _, errorClass := range errorClasses {
		ast.SetParentInChildren(errorClass)
		tracker.InsertNodeBefore(sf, topLevelStmt, errorClass, true, change.LeadingTriviaOptionNone)
	}

	// Replace the function
	if node.Kind == ast.KindFunctionDeclaration {
		tracker.ReplaceNode(sf, node, newDecl, nil)
	} else {
		replaceExpressionViaStatement(tracker, sf, node, newDecl)
	}
}

// findTopLevelStatement walks up the parent chain from node until it finds
// a node whose parent is the SourceFile.
func findTopLevelStatement(node *ast.Node) *ast.Node {
	current := node
	for current.Parent != nil && current.Parent.Kind != ast.KindSourceFile {
		current = current.Parent
	}
	return current
}

// buildYieldStarTryPromise builds: yield* Effect.tryPromise({ try: () => expr, catch: cause => new ErrorN({ cause }) })
func buildYieldStarTryPromise(tracker *change.Tracker, expr *ast.Node, effectModuleName string, errorName string) *ast.Node {
	// try: () => expr
	tryArrow := tracker.NewArrowFunction(
		nil, // modifiers
		nil, // typeParameters
		tracker.NewNodeList([]*ast.Node{}), // parameters (empty)
		nil, // returnType
		nil, // fullSignature
		tracker.NewToken(ast.KindEqualsGreaterThanToken),
		expr,
	)
	tryProp := tracker.NewPropertyAssignment(
		nil, // modifiers
		tracker.NewIdentifier("try"),
		nil, // postfixToken
		nil, // typeNode
		tryArrow,
	)

	// catch: cause => new ErrorN({ cause })
	causeParam := tracker.NewParameterDeclaration(
		nil, // modifiers
		nil, // dotDotDotToken
		tracker.NewIdentifier("cause"),
		nil, // questionToken
		nil, // typeNode
		nil, // initializer
	)

	// new ErrorN({ cause })
	causeShorthand := tracker.NewShorthandPropertyAssignment(
		nil, // modifiers
		tracker.NewIdentifier("cause"),
		nil, // postfixToken
		nil, // typeNode
		nil, // equalsToken
		nil, // objectAssignmentInitializer
	)
	objLiteral := tracker.NewObjectLiteralExpression(
		tracker.NewNodeList([]*ast.Node{causeShorthand}),
		false, // not multiline
	)
	newErrorExpr := tracker.NewNewExpression(
		tracker.NewIdentifier(errorName),
		nil, // typeArguments
		tracker.NewNodeList([]*ast.Node{objLiteral}),
	)

	catchArrow := tracker.NewArrowFunction(
		nil, // modifiers
		nil, // typeParameters
		tracker.NewNodeList([]*ast.Node{causeParam}),
		nil, // returnType
		nil, // fullSignature
		tracker.NewToken(ast.KindEqualsGreaterThanToken),
		newErrorExpr,
	)
	catchProp := tracker.NewPropertyAssignment(
		nil, // modifiers
		tracker.NewIdentifier("catch"),
		nil, // postfixToken
		nil, // typeNode
		catchArrow,
	)

	// { try: () => expr, catch: cause => new ErrorN({ cause }) }
	optionsObj := tracker.NewObjectLiteralExpression(
		tracker.NewNodeList([]*ast.Node{tryProp, catchProp}),
		false, // not multiline
	)

	// Effect.tryPromise({ ... })
	effectId := tracker.NewIdentifier(effectModuleName)
	tryPromiseAccess := tracker.NewPropertyAccessExpression(
		effectId, nil, tracker.NewIdentifier("tryPromise"), ast.NodeFlagsNone,
	)
	tryPromiseCall := tracker.NewCallExpression(
		tryPromiseAccess, nil, nil,
		tracker.NewNodeList([]*ast.Node{optionsObj}),
		ast.NodeFlagsNone,
	)

	// yield* Effect.tryPromise({ ... })
	return tracker.NewYieldExpression(
		tracker.NewToken(ast.KindAsteriskToken),
		tryPromiseCall,
	)
}

// buildTaggedErrorClass builds: class ErrorN extends Data.TaggedError("ErrorN")<{ cause: unknown }> {}
func buildTaggedErrorClass(tracker *change.Tracker, dataModuleName string, errorName string, isExported bool) *ast.Node {
	// Data.TaggedError("ErrorN")
	dataId := tracker.NewIdentifier(dataModuleName)
	taggedErrorAccess := tracker.NewPropertyAccessExpression(
		dataId, nil, tracker.NewIdentifier("TaggedError"), ast.NodeFlagsNone,
	)
	taggedErrorCall := tracker.NewCallExpression(
		taggedErrorAccess, nil, nil,
		tracker.NewNodeList([]*ast.Node{tracker.NewStringLiteral(errorName, 0)}),
		ast.NodeFlagsNone,
	)

	// <{ cause: unknown }>
	causePropSig := tracker.NewPropertySignatureDeclaration(
		nil, // modifiers
		tracker.NewIdentifier("cause"),
		nil, // postfixToken
		tracker.NewKeywordTypeNode(ast.KindUnknownKeyword),
		nil, // initializer
	)
	typeLiteral := tracker.NewTypeLiteralNode(
		tracker.NewNodeList([]*ast.Node{causePropSig}),
	)

	// Data.TaggedError("ErrorN")<{ cause: unknown }>
	exprWithTypeArgs := tracker.NewExpressionWithTypeArguments(
		taggedErrorCall,
		tracker.NewNodeList([]*ast.Node{typeLiteral}),
	)

	// extends clause
	heritageClause := tracker.NewHeritageClause(
		ast.KindExtendsKeyword,
		tracker.NewNodeList([]*ast.Node{exprWithTypeArgs}),
	)

	// Modifiers (export if needed)
	var modifiers *ast.ModifierList
	if isExported {
		modifiers = tracker.NewModifierList([]*ast.Node{
			tracker.NewModifier(ast.KindExportKeyword),
		})
	}

	// class ErrorN extends Data.TaggedError("ErrorN")<{ cause: unknown }> {}
	return tracker.NewClassDeclaration(
		modifiers,
		tracker.NewIdentifier(errorName),
		nil, // typeParameters
		tracker.NewNodeList([]*ast.Node{heritageClause}),
		tracker.NewNodeList([]*ast.Node{}), // empty members
	)
}
