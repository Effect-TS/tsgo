package refactors

import (
	"github.com/effect-ts/effect-typescript-go/internal/checkerutils"
	"github.com/effect-ts/effect-typescript-go/internal/refactor"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/astnav"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
)

var ToggleTypeAnnotation = refactor.Refactor{
	Name:        "toggleTypeAnnotation",
	Description: "Toggle type annotation",
	Kind:        "rewrite.effect.toggleTypeAnnotation",
	Run:         runToggleTypeAnnotation,
}

func runToggleTypeAnnotation(ctx *refactor.Context) []ls.CodeAction {
	token := astnav.GetTokenAtPosition(ctx.SourceFile, ctx.Span.Pos())
	if token == nil {
		return nil
	}

	// Walk ancestor chain looking for a VariableDeclaration or PropertyDeclaration
	var matchedNode *ast.Node
	for node := token; node != nil; node = node.Parent {
		if node.Kind != ast.KindVariableDeclaration && node.Kind != ast.KindPropertyDeclaration {
			continue
		}

		// Must have an initializer
		var initializer *ast.Node
		var name *ast.Node
		switch node.Kind {
		case ast.KindVariableDeclaration:
			vd := node.AsVariableDeclaration()
			initializer = vd.Initializer
			name = vd.Name()
		case ast.KindPropertyDeclaration:
			pd := node.AsPropertyDeclaration()
			initializer = pd.Initializer
			name = pd.Name()
		}

		if initializer == nil || name == nil {
			continue
		}

		// Cursor must be on the name
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

	// Get name and type nodes
	var name *ast.Node
	var typeNode *ast.Node
	var initializer *ast.Node
	switch matchedNode.Kind {
	case ast.KindVariableDeclaration:
		vd := matchedNode.AsVariableDeclaration()
		name = vd.Name()
		typeNode = vd.Type
		initializer = vd.Initializer
	case ast.KindPropertyDeclaration:
		pd := matchedNode.AsPropertyDeclaration()
		name = pd.Name()
		typeNode = pd.Type
		initializer = pd.Initializer
	}

	if typeNode != nil {
		// Remove existing type annotation: delete from name.End() to type.End()
		action := ctx.NewRefactorAction(refactor.RefactorAction{
			Description: "Toggle type annotation",
			Run: func(tracker *change.Tracker) {
				tracker.DeleteRange(ctx.SourceFile, core.NewTextRange(name.End(), typeNode.End()))
			},
		})
		if action == nil {
			return nil
		}
		action.Kind = "refactor.rewrite.effect.toggleTypeAnnotation"
		return []ls.CodeAction{*action}
	}

	// Add type annotation: infer type from initializer and insert after name
	c, done := ctx.GetTypeCheckerForFile(ctx.SourceFile)
	if c == nil {
		return nil
	}
	defer done()

	initializerType := checkerutils.GetTypeAtLocation(c, initializer)
	if initializerType == nil {
		return nil
	}

	typeStr := c.TypeToStringEx(initializerType, matchedNode, checker.TypeFormatFlagsNoTruncation)
	if typeStr == "" {
		return nil
	}

	action := ctx.NewRefactorAction(refactor.RefactorAction{
		Description: "Toggle type annotation",
		Run: func(tracker *change.Tracker) {
			tracker.InsertText(ctx.SourceFile, ctx.BytePosToLSPPosition(name.End()), ": "+typeStr)
		},
	})
	if action == nil {
		return nil
	}
	action.Kind = "refactor.rewrite.effect.toggleTypeAnnotation"
	return []ls.CodeAction{*action}
}
