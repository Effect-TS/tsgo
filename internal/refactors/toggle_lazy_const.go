package refactors

import (
	"github.com/effect-ts/effect-typescript-go/internal/refactor"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/astnav"
	"github.com/microsoft/typescript-go/shim/core"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
)

var ToggleLazyConst = refactor.Refactor{
	Name:        "toggleLazyConst",
	Description: "Toggle lazy const",
	Kind:        "rewrite.effect.toggleLazyConst",
	Run:         runToggleLazyConst,
}

func runToggleLazyConst(ctx *refactor.Context) []ls.CodeAction {
	// Only applicable with a non-empty selection
	if ctx.Span.Pos() == ctx.Span.End() {
		return nil
	}

	token := astnav.GetTokenAtPosition(ctx.SourceFile, ctx.Span.Pos())
	if token == nil {
		return nil
	}

	// Walk ancestor chain looking for a VariableDeclaration
	var matchedNode *ast.Node
	for node := token; node != nil; node = node.Parent {
		if node.Kind != ast.KindVariableDeclaration {
			continue
		}

		vd := node.AsVariableDeclaration()

		// Must have an initializer
		if vd.Initializer == nil {
			continue
		}

		// Cursor must be on the name
		name := vd.Name()
		if name == nil {
			continue
		}
		namePos := astnav.GetStartOfNode(name, ctx.SourceFile, false)
		nameEnd := name.End()
		if ctx.Span.Pos() < namePos || ctx.Span.Pos() > nameEnd {
			continue
		}

		// Exclude arrow functions with block bodies
		if vd.Initializer.Kind == ast.KindArrowFunction {
			af := vd.Initializer.AsArrowFunction()
			if af.Body != nil && af.Body.Kind == ast.KindBlock {
				return nil
			}
		}

		matchedNode = node
		break
	}

	if matchedNode == nil {
		return nil
	}

	initializer := matchedNode.AsVariableDeclaration().Initializer

	// Check if this is a zero-param arrow with expression body → unwrap
	if initializer.Kind == ast.KindArrowFunction {
		af := initializer.AsArrowFunction()
		if (af.Parameters == nil || len(af.Parameters.Nodes) == 0) && af.Body != nil {
			// Unwrap: remove the `() => ` prefix and any trailing content
			action := ctx.NewRefactorAction(refactor.RefactorAction{
				Description: "Toggle lazy const",
				Run: func(tracker *change.Tracker) {
					// Delete from body.end to initializer.end (trailing content after body)
					if af.Body.End() != initializer.End() {
						tracker.DeleteRange(ctx.SourceFile, core.NewTextRange(af.Body.End(), initializer.End()))
					}
					// Delete from initializer.pos to body.pos (the `() => ` prefix)
					tracker.DeleteRange(ctx.SourceFile, core.NewTextRange(initializer.Pos(), af.Body.Pos()))
				},
			})
			if action == nil {
				return nil
			}
			action.Kind = "refactor.rewrite.effect.toggleLazyConst"
			return []ls.CodeAction{*action}
		}
	}

	// Wrap: insert `() => ` before the initializer
	action := ctx.NewRefactorAction(refactor.RefactorAction{
		Description: "Toggle lazy const",
		Run: func(tracker *change.Tracker) {
			tracker.InsertText(ctx.SourceFile, ctx.BytePosToLSPPosition(initializer.Pos()), " () =>")
		},
	})
	if action == nil {
		return nil
	}
	action.Kind = "refactor.rewrite.effect.toggleLazyConst"
	return []ls.CodeAction{*action}
}
