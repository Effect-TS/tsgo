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

var AsyncAwaitToGen = refactor.Refactor{
	Name:        "asyncAwaitToGen",
	Description: "Convert async/await to Effect.gen",
	Kind:        "rewrite.effect.asyncAwaitToGen",
	Run:         runAsyncAwaitToGen,
}

func runAsyncAwaitToGen(ctx *refactor.Context) []ls.CodeAction {
	// Only applicable with a non-empty selection
	if ctx.Span.Pos() == ctx.Span.End() {
		return nil
	}

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

	action := ctx.NewRefactorAction(refactor.RefactorAction{
		Description: "Rewrite to Effect.gen",
		Run: func(tracker *change.Tracker) {
			newDecl := transformAsyncToEffectGen(tracker, asyncFn, effectModuleName)
			if newDecl != nil {
				ast.SetParentInChildren(newDecl)
				// For FunctionDeclaration (a statement), replace directly.
				// For FunctionExpression/ArrowFunction (not statements), find the
				// containing statement and replace at that level to avoid a printer
				// assertion failure with complex synthetic trees in expression context.
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

	action.Kind = "refactor.rewrite.effect.asyncAwaitToGen"
	return []ls.CodeAction{*action}
}

// replaceExpressionViaStatement replaces an expression node by finding its
// containing VariableStatement, rebuilding it with the new expression as
// initializer, and replacing the entire statement. This avoids a printer
// assertion failure that occurs when replacing non-statement nodes with
// complex synthetic trees.
func replaceExpressionViaStatement(tracker *change.Tracker, sf *ast.SourceFile, oldExpr *ast.Node, newExpr *ast.Node) {
	// Walk up to find containing VariableDeclaration
	varDecl := findParentOfKind(oldExpr, ast.KindVariableDeclaration)
	if varDecl == nil {
		// Fallback: replace directly
		tracker.ReplaceNode(sf, oldExpr, newExpr, nil)
		return
	}

	// Walk up to find containing VariableStatement
	varStmt := findParentOfKind(varDecl, ast.KindVariableStatement)
	if varStmt == nil {
		tracker.ReplaceNode(sf, oldExpr, newExpr, nil)
		return
	}

	// Rebuild the VariableDeclaration with new initializer
	vd := varDecl.AsVariableDeclaration()
	var name *ast.Node
	if vd.Name() != nil {
		name = tracker.DeepCloneNode(vd.Name())
	}
	var typeNode *ast.Node
	if vd.Type != nil {
		typeNode = tracker.DeepCloneNode(vd.Type)
	}
	newVarDecl := tracker.NewVariableDeclaration(name, nil, typeNode, newExpr)

	// Rebuild the VariableDeclarationList
	vdl := varDecl.Parent.AsVariableDeclarationList()
	newVarDeclList := tracker.NewVariableDeclarationList(vdl.Flags&ast.NodeFlagsBlockScoped, tracker.NewNodeList([]*ast.Node{newVarDecl}))

	// Rebuild the VariableStatement with cloned modifiers
	vs := varStmt.AsVariableStatement()
	var mods *ast.ModifierList
	if vs.Modifiers() != nil {
		var modNodes []*ast.Node
		for _, mod := range vs.Modifiers().Nodes {
			modNodes = append(modNodes, tracker.NewModifier(mod.Kind))
		}
		if len(modNodes) > 0 {
			mods = tracker.NewModifierList(modNodes)
		}
	}
	newVarStmt := tracker.NewVariableStatement(mods, newVarDeclList)

	ast.SetParentInChildren(newVarStmt)
	tracker.ReplaceNode(sf, varStmt, newVarStmt, nil)
}

// findParentOfKind walks up the parent chain to find a node of the given kind.
func findParentOfKind(node *ast.Node, kind ast.Kind) *ast.Node {
	for n := node.Parent; n != nil; n = n.Parent {
		if n.Kind == kind {
			return n
		}
	}
	return nil
}

// findAsyncFunction walks the ancestor chain from token to find the nearest
// async function declaration, function expression, or arrow function.
func findAsyncFunction(token *ast.Node) *ast.Node {
	for node := token; node != nil; node = node.Parent {
		switch node.Kind {
		case ast.KindFunctionDeclaration, ast.KindFunctionExpression, ast.KindArrowFunction:
			if ast.GetCombinedModifierFlags(node)&ast.ModifierFlagsAsync != 0 {
				if typeparser.GetFunctionLikeBody(node) != nil {
					return node
				}
			}
			// If we hit a non-async function boundary, stop looking
			return nil
		}
	}
	return nil
}

// transformAsyncToEffectGen transforms an async function to use Effect.gen.
func transformAsyncToEffectGen(tracker *change.Tracker, node *ast.Node, effectModuleName string) *ast.Node {
	body := typeparser.GetFunctionLikeBody(node)
	if body == nil {
		return nil
	}

	// Transform await expressions to yield* Effect.promise(...)
	transformedBody := transformBodyAwaitToYield(tracker, body, func(t *change.Tracker, expr *ast.Node) *ast.Node {
		return buildYieldStarPromise(t, expr, effectModuleName)
	})

	// Build Effect.gen(function*() { transformedBody })
	effectGenCall := buildEffectGenCall(tracker, transformedBody, effectModuleName)

	// Build non-async declaration wrapping the gen call
	return buildNonAsyncDeclaration(tracker, node, effectGenCall)
}

// TransformAwaitExpr is the callback type for transforming await expressions.
type TransformAwaitExpr func(tracker *change.Tracker, expr *ast.Node) *ast.Node

// transformBodyAwaitToYield replaces all AwaitExpression nodes in the body
// with the result of onAwait. Returns a deep-cloned, fully synthetic tree.
func transformBodyAwaitToYield(tracker *change.Tracker, body *ast.Node, onAwait TransformAwaitExpr) *ast.Node {
	// If no await expressions, just deep-clone the body
	if !containsAwaitExpression(body) {
		return tracker.DeepCloneNode(body)
	}

	// Visit the original body to transform await → yield*
	var v *ast.NodeVisitor
	visitFn := func(n *ast.Node) *ast.Node {
		if n.Kind == ast.KindAwaitExpression {
			awaitExpr := n.AsAwaitExpression()
			visitedInner := v.VisitNode(awaitExpr.Expression)
			return onAwait(tracker, visitedInner)
		}
		return v.VisitEachChild(n)
	}
	v = ast.NewNodeVisitor(visitFn, tracker.NodeFactory, ast.NodeVisitorHooks{})

	visited := v.VisitNode(body)

	// Deep-clone the result to make all nodes synthetic
	result := tracker.DeepCloneNode(visited)
	ast.SetParentInChildren(result)
	return result
}

// containsAwaitExpression checks if a node tree contains any AwaitExpression.
func containsAwaitExpression(node *ast.Node) bool {
	if node.Kind == ast.KindAwaitExpression {
		return true
	}
	found := false
	node.ForEachChild(func(child *ast.Node) bool {
		if containsAwaitExpression(child) {
			found = true
			return true // stop
		}
		return false
	})
	return found
}

// buildYieldStarPromise builds: yield* Effect.promise(() => expr)
func buildYieldStarPromise(tracker *change.Tracker, expr *ast.Node, effectModuleName string) *ast.Node {
	// () => expr
	arrowFn := tracker.NewArrowFunction(
		nil, // modifiers
		nil, // typeParameters
		tracker.NewNodeList([]*ast.Node{}), // parameters (empty)
		nil, // returnType
		nil, // fullSignature
		tracker.NewToken(ast.KindEqualsGreaterThanToken),
		expr,
	)

	// Effect.promise(() => expr)
	effectId := tracker.NewIdentifier(effectModuleName)
	promiseAccess := tracker.NewPropertyAccessExpression(
		effectId, nil, tracker.NewIdentifier("promise"), ast.NodeFlagsNone,
	)
	promiseCall := tracker.NewCallExpression(
		promiseAccess, nil, nil,
		tracker.NewNodeList([]*ast.Node{arrowFn}),
		ast.NodeFlagsNone,
	)

	// yield* Effect.promise(() => expr)
	return tracker.NewYieldExpression(
		tracker.NewToken(ast.KindAsteriskToken),
		promiseCall,
	)
}

// buildEffectGenCall builds: Effect.gen(function*() { body })
func buildEffectGenCall(tracker *change.Tracker, body *ast.Node, effectModuleName string) *ast.Node {
	var blockBody *ast.Node
	if body.Kind == ast.KindBlock {
		blockBody = body
	} else {
		// Expression body: wrap in { return <expr> }
		returnStmt := tracker.NewReturnStatement(body)
		blockBody = tracker.NewBlock(tracker.NewNodeList([]*ast.Node{returnStmt}), true)
	}

	// function*() { ... }
	genFn := tracker.NewFunctionExpression(
		nil,                                    // modifiers
		tracker.NewToken(ast.KindAsteriskToken), // asterisk (generator)
		nil,                                    // name
		nil,                                    // typeParameters
		tracker.NewNodeList([]*ast.Node{}),     // parameters (empty)
		nil,                                    // returnType
		nil,                                    // fullSignature
		blockBody,
	)

	// Effect.gen(function*() { ... })
	effectId := tracker.NewIdentifier(effectModuleName)
	genAccess := tracker.NewPropertyAccessExpression(
		effectId, nil, tracker.NewIdentifier("gen"), ast.NodeFlagsNone,
	)
	return tracker.NewCallExpression(
		genAccess, nil, nil,
		tracker.NewNodeList([]*ast.Node{genFn}),
		ast.NodeFlagsNone,
	)
}

// buildNonAsyncDeclaration builds a new function declaration/expression/arrow
// without the async modifier, with the given effectGenCall as the new body.
func buildNonAsyncDeclaration(tracker *change.Tracker, node *ast.Node, effectGenCall *ast.Node) *ast.Node {
	modifiers := getModifiersWithoutAsync(tracker, node)

	switch node.Kind {
	case ast.KindArrowFunction:
		af := node.AsArrowFunction()
		return tracker.NewArrowFunction(
			modifiers,
			cloneNodeList(tracker, af.TypeParameters),
			cloneNodeList(tracker, af.Parameters),
			nil, // returnType
			nil, // fullSignature
			tracker.NewToken(ast.KindEqualsGreaterThanToken),
			effectGenCall,
		)

	case ast.KindFunctionDeclaration:
		fd := node.AsFunctionDeclaration()
		returnStmt := tracker.NewReturnStatement(effectGenCall)
		newBody := tracker.NewBlock(tracker.NewNodeList([]*ast.Node{returnStmt}), true)
		var name *ast.Node
		if fd.Name() != nil {
			name = tracker.DeepCloneNode(fd.Name())
		}
		return tracker.NewFunctionDeclaration(
			modifiers,
			nil, // asteriskToken
			name,
			cloneNodeList(tracker, fd.TypeParameters),
			cloneNodeList(tracker, fd.Parameters),
			nil, // returnType
			nil, // fullSignature
			newBody,
		)

	case ast.KindFunctionExpression:
		fe := node.AsFunctionExpression()
		returnStmt := tracker.NewReturnStatement(effectGenCall)
		newBody := tracker.NewBlock(tracker.NewNodeList([]*ast.Node{returnStmt}), true)
		var name *ast.Node
		if fe.Name() != nil {
			name = tracker.DeepCloneNode(fe.Name())
		}
		return tracker.NewFunctionExpression(
			modifiers,
			nil, // asteriskToken
			name,
			cloneNodeList(tracker, fe.TypeParameters),
			cloneNodeList(tracker, fe.Parameters),
			nil, // returnType
			nil, // fullSignature
			newBody,
		)
	}

	return nil
}

// getModifiersWithoutAsync returns a new modifier list with the async keyword removed.
func getModifiersWithoutAsync(tracker *change.Tracker, node *ast.Node) *ast.ModifierList {
	var srcModifiers *ast.ModifierList
	switch node.Kind {
	case ast.KindFunctionDeclaration:
		srcModifiers = node.AsFunctionDeclaration().Modifiers()
	case ast.KindFunctionExpression:
		srcModifiers = node.AsFunctionExpression().Modifiers()
	case ast.KindArrowFunction:
		srcModifiers = node.AsArrowFunction().Modifiers()
	}

	if srcModifiers == nil {
		return nil
	}

	var modNodes []*ast.Node
	for _, mod := range srcModifiers.Nodes {
		if mod.Kind == ast.KindAsyncKeyword {
			continue
		}
		modNodes = append(modNodes, tracker.NewModifier(mod.Kind))
	}

	if len(modNodes) == 0 {
		return nil
	}
	return tracker.NewModifierList(modNodes)
}

// cloneNodeList deep-clones a NodeList if non-nil.
func cloneNodeList(tracker *change.Tracker, list *ast.NodeList) *ast.NodeList {
	if list == nil {
		return nil
	}
	if len(list.Nodes) == 0 {
		return tracker.NewNodeList([]*ast.Node{})
	}
	cloned := make([]*ast.Node, len(list.Nodes))
	for i, n := range list.Nodes {
		cloned[i] = tracker.DeepCloneNode(n)
	}
	return tracker.NewNodeList(cloned)
}
