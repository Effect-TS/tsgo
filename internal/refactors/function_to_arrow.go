package refactors

import (
	"github.com/effect-ts/effect-typescript-go/internal/refactor"
	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/astnav"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
)

var FunctionToArrow = refactor.Refactor{
	Name:        "functionToArrow",
	Description: "Convert to arrow",
	Kind:        "rewrite.effect.functionToArrow",
	Run:         runFunctionToArrow,
}

func runFunctionToArrow(ctx *refactor.Context) []ls.CodeAction {
	// Only applicable with a non-empty selection
	if ctx.Span.Pos() == ctx.Span.End() {
		return nil
	}

	token := astnav.GetTokenAtPosition(ctx.SourceFile, ctx.Span.Pos())
	if token == nil {
		return nil
	}

	// Walk ancestor chain looking for a FunctionDeclaration or MethodDeclaration
	var matchedNode *ast.Node
	for node := token; node != nil; node = node.Parent {
		if node.Kind != ast.KindFunctionDeclaration && node.Kind != ast.KindMethodDeclaration {
			continue
		}

		// Must have a body
		body := typeparser.GetFunctionLikeBody(node)
		if body == nil {
			continue
		}

		// Exclude async functions (those are handled by asyncAwaitToFn/asyncAwaitToGen)
		if ast.GetCombinedModifierFlags(node)&ast.ModifierFlagsAsync != 0 {
			continue
		}

		// Must have a name and cursor must be on it
		var name *ast.Node
		switch node.Kind {
		case ast.KindFunctionDeclaration:
			name = node.AsFunctionDeclaration().Name()
		case ast.KindMethodDeclaration:
			name = node.AsMethodDeclaration().Name()
		}
		if name == nil {
			continue
		}

		namePos := astnav.GetStartOfNode(name, ctx.SourceFile, false)
		nameEnd := name.End()
		if ctx.Span.Pos() < namePos || ctx.Span.Pos() > nameEnd {
			continue
		}

		matchedNode = node
		break
	}

	if matchedNode == nil {
		return nil
	}

	action := ctx.NewRefactorAction(refactor.RefactorAction{
		Description: "Convert to arrow",
		Run: func(tracker *change.Tracker) {
			newNode := buildFunctionToArrowReplacement(tracker, matchedNode)
			if newNode == nil {
				return
			}
			ast.SetParentInChildren(newNode)
			tracker.ReplaceNode(ctx.SourceFile, matchedNode, newNode, nil)
		},
	})
	if action == nil {
		return nil
	}
	action.Kind = "refactor.rewrite.effect.functionToArrow"
	return []ls.CodeAction{*action}
}

// buildFunctionToArrowReplacement builds the replacement node for converting a
// FunctionDeclaration or MethodDeclaration to an arrow function.
func buildFunctionToArrowReplacement(tracker *change.Tracker, node *ast.Node) *ast.Node {
	body := typeparser.GetFunctionLikeBody(node)
	if body == nil {
		return nil
	}

	// Determine concise vs block body:
	// If body has exactly one ReturnStatement with an expression, use concise body
	var newBody *ast.Node
	if body.Kind == ast.KindBlock {
		block := body.AsBlock()
		if block.Statements != nil && len(block.Statements.Nodes) == 1 {
			stmt := block.Statements.Nodes[0]
			if stmt.Kind == ast.KindReturnStatement {
				ret := stmt.AsReturnStatement()
				if ret.Expression != nil {
					// Concise body: just the expression
					newBody = tracker.DeepCloneNode(ret.Expression)
				}
			}
		}
		if newBody == nil {
			// Block body
			newBody = tracker.DeepCloneNode(body)
		}
	} else {
		newBody = tracker.DeepCloneNode(body)
	}

	// Build arrow modifiers: strip Export and Default from the arrow itself
	var arrowMods *ast.ModifierList
	srcMods := getNodeModifiers(node)
	if srcMods != nil {
		var modNodes []*ast.Node
		for _, mod := range srcMods.Nodes {
			if mod.Kind == ast.KindExportKeyword || mod.Kind == ast.KindDefaultKeyword {
				continue
			}
			modNodes = append(modNodes, tracker.NewModifier(mod.Kind))
		}
		if len(modNodes) > 0 {
			arrowMods = tracker.NewModifierList(modNodes)
		}
	}

	// Build the arrow function
	typeParams := cloneNodeList(tracker, typeparser.GetFunctionLikeTypeParameters(node))
	params := cloneNodeList(tracker, typeparser.GetFunctionLikeParameters(node))

	arrowFn := tracker.NewArrowFunction(
		arrowMods,
		typeParams,
		params,
		nil, // returnType
		nil, // fullSignature
		tracker.NewToken(ast.KindEqualsGreaterThanToken),
		newBody,
	)

	// Wrap in appropriate declaration
	switch node.Kind {
	case ast.KindFunctionDeclaration:
		return wrapArrowInVariableStatement(tracker, node, arrowFn)
	case ast.KindMethodDeclaration:
		return wrapArrowInPropertyDeclaration(tracker, node, arrowFn)
	}

	return arrowFn
}

// wrapArrowInVariableStatement wraps an arrow function in a const variable statement,
// preserving export/default modifiers on the variable statement.
func wrapArrowInVariableStatement(tracker *change.Tracker, node *ast.Node, arrowFn *ast.Node) *ast.Node {
	fd := node.AsFunctionDeclaration()
	if fd.Name() == nil {
		return arrowFn
	}

	// Collect modifiers for the variable statement (keep all original modifiers)
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

	varDecl := tracker.NewVariableDeclaration(
		tracker.DeepCloneNode(fd.Name()),
		nil, // exclamationToken
		nil, // type
		arrowFn,
	)
	varDeclList := tracker.NewVariableDeclarationList(
		ast.NodeFlagsConst,
		tracker.NewNodeList([]*ast.Node{varDecl}),
	)
	return tracker.NewVariableStatement(mods, varDeclList)
}

// wrapArrowInPropertyDeclaration wraps an arrow function in a property declaration,
// preserving modifiers from the method.
func wrapArrowInPropertyDeclaration(tracker *change.Tracker, node *ast.Node, arrowFn *ast.Node) *ast.Node {
	md := node.AsMethodDeclaration()

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
		arrowFn,
	)
}

// getNodeModifiers returns the modifier list from a function-like node.
func getNodeModifiers(node *ast.Node) *ast.ModifierList {
	switch node.Kind {
	case ast.KindFunctionDeclaration:
		return node.AsFunctionDeclaration().Modifiers()
	case ast.KindMethodDeclaration:
		return node.AsMethodDeclaration().Modifiers()
	}
	return nil
}
