package refactors

import (
	"github.com/effect-ts/tsgo/internal/refactor"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/astnav"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
)

var AsyncAwaitToFn = refactor.Refactor{
	Name:        "asyncAwaitToFn",
	Description: "Convert async/await to Effect.fn",
	Kind:        "rewrite.effect.asyncAwaitToFn",
	Run:         runAsyncAwaitToFn,
}

func runAsyncAwaitToFn(ctx *refactor.Context) []ls.CodeAction {
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

	action := ctx.NewRefactorAction(refactor.RefactorAction{
		Description: "Rewrite to Effect.fn",
		Run: func(tracker *change.Tracker) {
			newDecl := transformAsyncToEffectFn(tracker, asyncFn, effectModuleName)
			if newDecl != nil {
				ast.SetParentInChildren(newDecl)
				// For function declarations, we replace the whole declaration with a variable statement
				if asyncFn.Kind == ast.KindFunctionDeclaration {
					tracker.ReplaceNode(ctx.SourceFile, asyncFn, newDecl, nil)
				} else {
					replaceExpressionViaStatement(tracker, ctx.SourceFile, asyncFn, newDecl)
				}
			}
		},
	})
	if action == nil {
		return nil
	}

	action.Kind = "refactor.rewrite.effect.asyncAwaitToFn"
	return []ls.CodeAction{*action}
}

// transformAsyncToEffectFn transforms an async function to use Effect.fn.
func transformAsyncToEffectFn(tracker *change.Tracker, node *ast.Node, effectModuleName string) *ast.Node {
	body := typeparser.GetFunctionLikeBody(node)
	if body == nil {
		return nil
	}

	// Transform await expressions to yield* Effect.promise(...)
	transformedBody := transformBodyAwaitToYield(tracker, body, func(t *change.Tracker, expr *ast.Node) *ast.Node {
		return buildYieldStarPromise(t, expr, effectModuleName)
	})

	// Get the function name for the trace string
	fnName := typeparser.GetFunctionLikeName(node)

	// Build Effect.fn("name")(function*(params) { body })
	effectFnCall := buildEffectFnCall(tracker, node, transformedBody, effectModuleName, fnName)

	// Build declaration wrapping the fn call
	return buildFnDeclaration(tracker, node, effectFnCall, fnName)
}

// buildEffectFnCall builds: Effect.fn("name")(function*(params) { body })
func buildEffectFnCall(tracker *change.Tracker, node *ast.Node, body *ast.Node, effectModuleName string, fnName string) *ast.Node {
	var blockBody *ast.Node
	if body.Kind == ast.KindBlock {
		blockBody = body
	} else {
		// Expression body: wrap in { return <expr> }
		returnStmt := tracker.NewReturnStatement(body)
		blockBody = tracker.NewBlock(tracker.NewNodeList([]*ast.Node{returnStmt}), true)
	}

	// Get type parameters and parameters from original function
	var typeParams *ast.NodeList
	var params *ast.NodeList

	switch node.Kind {
	case ast.KindArrowFunction:
		af := node.AsArrowFunction()
		typeParams = cloneNodeList(tracker, af.TypeParameters)
		params = cloneNodeList(tracker, af.Parameters)
	case ast.KindFunctionDeclaration:
		fd := node.AsFunctionDeclaration()
		typeParams = cloneNodeList(tracker, fd.TypeParameters)
		params = cloneNodeList(tracker, fd.Parameters)
	case ast.KindFunctionExpression:
		fe := node.AsFunctionExpression()
		typeParams = cloneNodeList(tracker, fe.TypeParameters)
		params = cloneNodeList(tracker, fe.Parameters)
	}

	if params == nil {
		params = tracker.NewNodeList([]*ast.Node{})
	}

	// function*(params) { ... }
	genFn := tracker.NewFunctionExpression(
		nil,                                     // modifiers
		tracker.NewToken(ast.KindAsteriskToken), // asterisk (generator)
		nil,                                     // name
		typeParams,                              // typeParameters
		params,                                  // parameters
		nil,                                     // returnType
		nil,                                     // fullSignature
		blockBody,
	)

	// Build Effect.fn or Effect.fn("name")
	effectId := tracker.NewIdentifier(effectModuleName)
	fnAccess := tracker.NewPropertyAccessExpression(
		effectId, nil, tracker.NewIdentifier("fn"), ast.NodeFlagsNone,
	)

	var fnCallExpr *ast.Node
	if fnName != "" {
		// Effect.fn("name")
		fnCallExpr = tracker.NewCallExpression(
			fnAccess, nil, nil,
			tracker.NewNodeList([]*ast.Node{tracker.NewStringLiteral(fnName, 0)}),
			ast.NodeFlagsNone,
		)
	} else {
		fnCallExpr = fnAccess
	}

	// Effect.fn("name")(function*(params) { ... })
	return tracker.NewCallExpression(
		fnCallExpr, nil, nil,
		tracker.NewNodeList([]*ast.Node{genFn}),
		ast.NodeFlagsNone,
	)
}

// buildFnDeclaration builds the appropriate declaration wrapping the Effect.fn call.
// For function declarations: const name = Effect.fn("name")(...)
// For expressions/arrows: just the Effect.fn("name")(...) expression
func buildFnDeclaration(tracker *change.Tracker, node *ast.Node, effectFnCall *ast.Node, _ string) *ast.Node {
	if node.Kind == ast.KindFunctionDeclaration {
		fd := node.AsFunctionDeclaration()
		if fd.Name() == nil {
			return effectFnCall
		}

		// Build modifiers without async
		modifiers := getModifiersWithoutAsync(tracker, node)

		// const name = Effect.fn("name")(...)
		varDecl := tracker.NewVariableDeclaration(
			tracker.DeepCloneNode(fd.Name()),
			nil, // exclamationToken
			nil, // type
			effectFnCall,
		)
		varDeclList := tracker.NewVariableDeclarationList(
			tracker.NewNodeList([]*ast.Node{varDecl}),
			ast.NodeFlagsConst,
		)
		return tracker.NewVariableStatement(modifiers, varDeclList)
	}

	// For arrow functions and function expressions, return the expression directly
	return effectFnCall
}
