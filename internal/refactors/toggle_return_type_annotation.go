package refactors

import (
	"github.com/effect-ts/effect-typescript-go/internal/refactor"
	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/astnav"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
)

var ToggleReturnTypeAnnotation = refactor.Refactor{
	Name:        "toggleReturnTypeAnnotation",
	Description: "Toggle return type annotation",
	Kind:        "rewrite.effect.toggleReturnTypeAnnotation",
	Run:         runToggleReturnTypeAnnotation,
}

func runToggleReturnTypeAnnotation(ctx *refactor.Context) []ls.CodeAction {
	token := astnav.GetTokenAtPosition(ctx.SourceFile, ctx.Span.Pos())
	if token == nil {
		return nil
	}

	// Walk ancestor chain looking for a function-like declaration
	var matchedNode *ast.Node
	for node := token; node != nil; node = node.Parent {
		switch node.Kind {
		case ast.KindFunctionDeclaration, ast.KindFunctionExpression, ast.KindArrowFunction, ast.KindMethodDeclaration:
			matchedNode = node
		default:
			continue
		}
		break
	}

	if matchedNode == nil {
		return nil
	}

	// Get the type node and close paren position
	typeNode := getFunctionLikeType(matchedNode)
	closeParen := astnav.FindChildOfKind(matchedNode, ast.KindCloseParenToken, ctx.SourceFile)

	// For arrow functions without parens (e.g., x => ...), use the last parameter end
	var endNode *ast.Node
	if closeParen != nil {
		endNode = closeParen
	} else if matchedNode.Kind == ast.KindArrowFunction {
		params := matchedNode.AsArrowFunction().Parameters
		if params != nil && len(params.Nodes) > 0 {
			endNode = params.Nodes[len(params.Nodes)-1]
		}
	}

	if endNode == nil {
		return nil
	}

	if typeNode != nil {
		// Remove existing return type annotation: delete from endNode.End() to type.End()
		action := ctx.NewRefactorAction(refactor.RefactorAction{
			Description: "Toggle return type annotation",
			Run: func(tracker *change.Tracker) {
				tracker.DeleteRange(ctx.SourceFile, core.NewTextRange(endNode.End(), typeNode.End()))
			},
		})
		if action == nil {
			return nil
		}
		action.Kind = "refactor.rewrite.effect.toggleReturnTypeAnnotation"
		return []ls.CodeAction{*action}
	}

	// Add return type annotation: infer return type and insert after close paren
	// Must have a body to infer from
	bodyData := matchedNode.BodyData()
	if bodyData == nil || bodyData.Body == nil {
		return nil
	}

	c, done := ctx.GetTypeCheckerForFile(ctx.SourceFile)
	if c == nil {
		return nil
	}
	defer done()

	returnType := getInferredReturnTypeFromChecker(c, matchedNode)
	if returnType == nil {
		return nil
	}

	typeStr := c.TypeToStringEx(returnType, matchedNode, checker.TypeFormatFlagsNoTruncation)
	if typeStr == "" {
		return nil
	}

	action := ctx.NewRefactorAction(refactor.RefactorAction{
		Description: "Toggle return type annotation",
		Run: func(tracker *change.Tracker) {
			tracker.InsertText(ctx.SourceFile, ctx.BytePosToLSPPosition(endNode.End()), ": "+typeStr)
		},
	})
	if action == nil {
		return nil
	}
	action.Kind = "refactor.rewrite.effect.toggleReturnTypeAnnotation"
	return []ls.CodeAction{*action}
}

// getFunctionLikeType returns the return type node from a function-like declaration.
func getFunctionLikeType(node *ast.Node) *ast.Node {
	switch node.Kind {
	case ast.KindFunctionDeclaration:
		return node.AsFunctionDeclaration().Type
	case ast.KindFunctionExpression:
		return node.AsFunctionExpression().Type
	case ast.KindArrowFunction:
		return node.AsArrowFunction().Type
	case ast.KindMethodDeclaration:
		return node.AsMethodDeclaration().Type
	}
	return nil
}

// getInferredReturnTypeFromChecker extracts the return type from a function-like declaration
// using the type checker. It handles overloaded functions and regular signatures.
func getInferredReturnTypeFromChecker(c *checker.Checker, declaration *ast.Node) *checker.Type {
	var returnType *checker.Type

	// Try overloaded function handling
	declType := typeparser.GetTypeAtLocation(c, declaration)
	if declType != nil {
		signatures := c.GetSignaturesOfType(declType, checker.SignatureKindCall)
		if len(signatures) > 1 {
			var returnTypes []*checker.Type
			for _, sig := range signatures {
				rt := c.GetReturnTypeOfSignature(sig)
				if rt != nil {
					returnTypes = append(returnTypes, rt)
				}
			}
			if len(returnTypes) > 0 {
				returnType = c.GetUnionType(returnTypes)
			}
		}
	}

	if returnType == nil {
		sig := c.GetSignatureFromDeclaration(declaration)
		if sig != nil {
			returnType = c.GetReturnTypeOfSignature(sig)
		}
	}

	return returnType
}
