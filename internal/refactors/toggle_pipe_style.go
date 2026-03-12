package refactors

import (
	"github.com/effect-ts/effect-typescript-go/internal/checkerutils"
	"github.com/effect-ts/effect-typescript-go/internal/refactor"
	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/astnav"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
)

var TogglePipeStyle = refactor.Refactor{
	Name:        "togglePipeStyle",
	Description: "Toggle pipe style",
	Kind:        "rewrite.effect.togglePipeStyle",
	Run:         runTogglePipeStyle,
}

func runTogglePipeStyle(ctx *refactor.Context) []ls.CodeAction {
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

	// Walk up the ancestor chain looking for a pipe call
	for node := token; node != nil; node = node.Parent {
		if node.Kind != ast.KindCallExpression {
			continue
		}

		pipeCall := typeparser.ParsePipeCall(c, node)
		if pipeCall == nil {
			continue
		}

		switch pipeCall.Kind {
		case typeparser.TransformationKindPipe:
			// pipe(subject, f1, f2) -> subject.pipe(f1, f2)
			// Check that the subject's type is pipeable
			subjectType := checkerutils.GetTypeAtLocation(c, pipeCall.Subject)
			if !typeparser.IsPipeableType(c, subjectType, pipeCall.Subject) {
				continue
			}

			action := ctx.NewRefactorAction(refactor.RefactorAction{
				Description: "Rewrite as X.pipe(Y, Z, ...)",
				Run: func(tracker *change.Tracker) {
					clonedSubject := tracker.DeepCloneNode(pipeCall.Subject)
					pipeAccess := tracker.NewPropertyAccessExpression(clonedSubject, nil, tracker.NewIdentifier("pipe"), ast.NodeFlagsNone)

					var clonedArgs []*ast.Node
					for _, arg := range pipeCall.Args {
						clonedArgs = append(clonedArgs, tracker.DeepCloneNode(arg))
					}

					callExpr := tracker.NewCallExpression(pipeAccess, nil, nil, tracker.NewNodeList(clonedArgs), ast.NodeFlagsNone)
					ast.SetParentInChildren(callExpr)
					tracker.ReplaceNode(ctx.SourceFile, node, callExpr, nil)
				},
			})
			if action == nil {
				return nil
			}
			action.Kind = "refactor.rewrite.effect.togglePipeStyle"
			return []ls.CodeAction{*action}

		case typeparser.TransformationKindPipeable:
			// subject.pipe(f1, f2) -> pipe(subject, f1, f2)
			action := ctx.NewRefactorAction(refactor.RefactorAction{
				Description: "Rewrite as pipe(X, Y, Z, ...)",
				Run: func(tracker *change.Tracker) {
					clonedSubject := tracker.DeepCloneNode(pipeCall.Subject)

					allArgs := make([]*ast.Node, 0, 1+len(pipeCall.Args))
					allArgs = append(allArgs, clonedSubject)
					for _, arg := range pipeCall.Args {
						allArgs = append(allArgs, tracker.DeepCloneNode(arg))
					}

					pipeId := tracker.NewIdentifier("pipe")
					callExpr := tracker.NewCallExpression(pipeId, nil, nil, tracker.NewNodeList(allArgs), ast.NodeFlagsNone)
					ast.SetParentInChildren(callExpr)
					tracker.ReplaceNode(ctx.SourceFile, node, callExpr, nil)
				},
			})
			if action == nil {
				return nil
			}
			action.Kind = "refactor.rewrite.effect.togglePipeStyle"
			return []ls.CodeAction{*action}
		}
	}

	return nil
}
