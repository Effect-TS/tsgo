package refactors

import (
	"github.com/effect-ts/tsgo/internal/refactor"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/astnav"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
)

var EffectGenToFn = refactor.Refactor{
	Name:        "effectGenToFn",
	Description: "Convert to Effect.fn",
	Kind:        "rewrite.effect.effectGenToFn",
	Run:         runEffectGenToFn,
}

// effectGenParsed holds the parsed components of a function returning Effect.gen.
type effectGenParsed struct {
	funcNode *ast.Node                       // The function-like node (arrow, declaration, expression, method)
	genCall  *typeparser.EffectGenCallResult // The parsed Effect.gen call
	pipeArgs []*ast.Node                     // Collected pipe arguments (from .pipe() or pipe() wrappers)
}

func runEffectGenToFn(ctx *refactor.Context) []ls.CodeAction {
	c := ctx.Checker

	token := astnav.GetTokenAtPosition(ctx.SourceFile, ctx.Span.Pos())
	if token == nil {
		return nil
	}

	// Walk ancestor chain to find a function-like node returning Effect.gen
	parsed := findFunctionReturningEffectGen(c, token)
	if parsed == nil {
		return nil
	}

	fnName := typeparser.GetFunctionLikeName(parsed.funcNode)

	description := "Convert to Effect.fn"
	if fnName != "" {
		description = "Convert to Effect.fn(\"" + fnName + "\")"
	}

	action := ctx.NewRefactorAction(refactor.RefactorAction{
		Description: description,
		Run: func(tracker *change.Tracker) {
			newNode := buildEffectGenToFnReplacement(tracker, parsed, fnName)
			if newNode == nil {
				return
			}
			ast.SetParentInChildren(newNode)

			switch parsed.funcNode.Kind {
			case ast.KindFunctionDeclaration, ast.KindMethodDeclaration:
				tracker.ReplaceNode(ctx.SourceFile, parsed.funcNode, newNode, nil)
			default:
				replaceExpressionViaStatement(tracker, ctx.SourceFile, parsed.funcNode, newNode)
			}
		},
	})
	if action == nil {
		return nil
	}

	action.Kind = "refactor.rewrite.effect.effectGenToFn"
	return []ls.CodeAction{*action}
}

// findFunctionReturningEffectGen walks from the token up the ancestor chain to find
// a function-like node whose body directly returns an Effect.gen call, optionally
// wrapped in pipe/pipeable chains.
func findFunctionReturningEffectGen(c *checker.Checker, token *ast.Node) *effectGenParsed {
	for node := token; node != nil; node = node.Parent {
		switch node.Kind {
		case ast.KindArrowFunction, ast.KindFunctionDeclaration, ast.KindMethodDeclaration:
			parsed := parseFunctionReturningEffectGen(c, node)
			if parsed != nil {
				return parsed
			}
			// Hit a function boundary without finding Effect.gen — stop
			return nil
		case ast.KindFunctionExpression:
			// Don't match on generator functions (those are inside Effect.gen already)
			fe := node.AsFunctionExpression()
			if fe.AsteriskToken != nil {
				continue
			}
			parsed := parseFunctionReturningEffectGen(c, node)
			if parsed != nil {
				return parsed
			}
			return nil
		}
	}
	return nil
}

// parseFunctionReturningEffectGen extracts the Effect.gen call and any pipe args
// from a function-like node. Returns nil if the function doesn't match.
func parseFunctionReturningEffectGen(c *checker.Checker, node *ast.Node) *effectGenParsed {
	body := typeparser.GetFunctionLikeBody(node)
	if body == nil {
		return nil
	}

	// Skip through { return <expr> } blocks to get the actual expression
	subject := skipReturnBlock(body)

	// Peel off pipe wrappers, collecting args
	var pipeArgs []*ast.Node
	for {
		pipeResult := typeparser.ParsePipeCall(c, subject)
		if pipeResult == nil {
			break
		}
		// Prepend args (inner pipes go first)
		pipeArgs = append(pipeResult.Args, pipeArgs...)
		subject = pipeResult.Subject
	}

	// Check if the core expression is Effect.gen(function*() { ... })
	genCall := typeparser.EffectGenCall(c, subject)
	if genCall == nil {
		return nil
	}

	return &effectGenParsed{
		funcNode: node,
		genCall:  genCall,
		pipeArgs: pipeArgs,
	}
}

// skipReturnBlock unwraps a block containing a single return statement to get the expression.
func skipReturnBlock(node *ast.Node) *ast.Node {
	if node.Kind != ast.KindBlock {
		return node
	}
	block := node.AsBlock()
	if block.Statements == nil || len(block.Statements.Nodes) != 1 {
		return node
	}
	stmt := block.Statements.Nodes[0]
	if stmt.Kind != ast.KindReturnStatement {
		return node
	}
	ret := stmt.AsReturnStatement()
	if ret.Expression == nil {
		return node
	}
	return ret.Expression
}

// buildEffectGenToFnReplacement builds the replacement node for the effectGenToFn refactor.
func buildEffectGenToFnReplacement(tracker *change.Tracker, parsed *effectGenParsed, fnName string) *ast.Node {
	node := parsed.funcNode
	genCall := parsed.genCall

	// Get the generator body
	var blockBody *ast.Node
	if genCall.Body != nil {
		body := genCall.Body
		if body.Kind == ast.KindBlock {
			blockBody = tracker.DeepCloneNode(body)
		} else {
			// Expression body: wrap in { return <expr> }
			clonedExpr := tracker.DeepCloneNode(body)
			returnStmt := tracker.NewReturnStatement(clonedExpr)
			blockBody = tracker.NewBlock(tracker.NewNodeList([]*ast.Node{returnStmt}), true)
		}
	}
	if blockBody == nil {
		return nil
	}

	// Get type parameters and parameters from the outer function
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
	case ast.KindMethodDeclaration:
		md := node.AsMethodDeclaration()
		typeParams = cloneNodeList(tracker, md.TypeParameters)
		params = cloneNodeList(tracker, md.Parameters)
	}

	// Build function*(params) { body }
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
	effectId := tracker.DeepCloneNode(genCall.EffectModule)
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

	// Build the arguments: [generatorFn, ...pipeArgs]
	args := []*ast.Node{genFn}
	for _, pipeArg := range parsed.pipeArgs {
		args = append(args, tracker.DeepCloneNode(pipeArg))
	}

	// Effect.fn("name")(function*(params) { body }, ...pipeArgs)
	effectFnCall := tracker.NewCallExpression(
		fnCallExpr, nil, nil,
		tracker.NewNodeList(args),
		ast.NodeFlagsNone,
	)

	// Wrap in appropriate declaration
	return buildEffectGenToFnDeclaration(tracker, node, effectFnCall)
}

// buildEffectGenToFnDeclaration wraps the Effect.fn call in the appropriate declaration.
func buildEffectGenToFnDeclaration(tracker *change.Tracker, node *ast.Node, effectFnCall *ast.Node) *ast.Node {
	switch node.Kind {
	case ast.KindFunctionDeclaration:
		fd := node.AsFunctionDeclaration()
		if fd.Name() == nil {
			return effectFnCall
		}

		// Collect modifiers
		var mods *ast.ModifierList
		if fd.Modifiers() != nil {
			var modNodes []*ast.Node
			for _, mod := range fd.Modifiers().Nodes {
				modNodes = append(modNodes, tracker.NewModifier(mod.Kind))
			}
			if len(modNodes) > 0 {
				mods = tracker.NewModifierList(modNodes)
			}
		}

		// const name = Effect.fn("name")(...)
		varDecl := tracker.NewVariableDeclaration(
			tracker.DeepCloneNode(fd.Name()),
			nil, // exclamationToken
			nil, // type
			effectFnCall,
		)
		varDeclList := tracker.NewVariableDeclarationList(
			ast.NodeFlagsConst,
			tracker.NewNodeList([]*ast.Node{varDecl}),
		)
		return tracker.NewVariableStatement(mods, varDeclList)

	case ast.KindMethodDeclaration:
		md := node.AsMethodDeclaration()
		// Convert method to property declaration: name = Effect.fn("name")(...)
		var mods *ast.ModifierList
		if md.Modifiers() != nil {
			var modNodes []*ast.Node
			for _, mod := range md.Modifiers().Nodes {
				modNodes = append(modNodes, tracker.NewModifier(mod.Kind))
			}
			if len(modNodes) > 0 {
				mods = tracker.NewModifierList(modNodes)
			}
		}
		return tracker.NewPropertyDeclaration(
			mods,
			tracker.DeepCloneNode(md.Name()),
			nil, // postfixToken
			nil, // type
			effectFnCall,
		)
	}

	// For arrow functions and function expressions, return the expression directly
	return effectFnCall
}
