package refactors

import (
	"strings"

	"github.com/effect-ts/tsgo/internal/refactor"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/astnav"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
	"github.com/microsoft/typescript-go/shim/scanner"
)

var TogglePipeStyle = refactor.Refactor{
	Name:        "togglePipeStyle",
	Description: "Toggle pipe style",
	Kind:        "rewrite.effect.togglePipeStyle",
	Run:         runTogglePipeStyle,
}

func runTogglePipeStyle(ctx *refactor.Context) []ls.CodeAction {
	token := astnav.GetTokenAtPosition(ctx.SourceFile, ctx.Span.Pos())
	if token == nil {
		return nil
	}

	// Walk up the ancestor chain looking for a pipe call
	for node := token; node != nil; node = node.Parent {
		if node.Kind != ast.KindCallExpression {
			continue
		}

		pipeCall := ctx.TypeParser.ParsePipeCall(node)
		if pipeCall == nil {
			continue
		}

		switch pipeCall.Kind {
		case typeparser.TransformationKindPipe:
			// pipe(subject, f1, f2) -> subject.pipe(f1, f2)
			// Check that the subject's type is pipeable
			subjectType := ctx.TypeParser.GetTypeAtLocation(pipeCall.Subject)
			if !ctx.TypeParser.IsPipeableType(subjectType, pipeCall.Subject) {
				continue
			}

			action := ctx.NewRefactorAction(refactor.RefactorAction{
				Description: "Rewrite as X.pipe(Y, Z, ...)",
				Run: func(tracker *rewriter.Tracker) {
					start := astnav.GetStartOfNode(node, ctx.SourceFile, false)
					rewritten := togglePipeToMethodText(ctx.SourceFile, pipeCall.Subject, pipeCall.Args)
					tracker.ReplaceRangeWithText(ctx.SourceFile, lsproto.Range{
						Start: ctx.BytePosToLSPPosition(start),
						End:   ctx.BytePosToLSPPosition(node.End()),
					}, rewritten)
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
				Run: func(tracker *rewriter.Tracker) {
					start := astnav.GetStartOfNode(node, ctx.SourceFile, false)
					rewritten := togglePipeToFunctionText(ctx.SourceFile, pipeCall.Subject, pipeCall.Args)
					tracker.ReplaceRangeWithText(ctx.SourceFile, lsproto.Range{
						Start: ctx.BytePosToLSPPosition(start),
						End:   ctx.BytePosToLSPPosition(node.End()),
					}, rewritten)
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

func togglePipeToMethodText(sf *ast.SourceFile, subject *ast.Node, args []*ast.Node) string {
	parts := make([]string, 0, len(args))
	for _, arg := range args {
		parts = append(parts, scanner.GetSourceTextOfNodeFromSourceFile(sf, arg, false))
	}
	return scanner.GetSourceTextOfNodeFromSourceFile(sf, subject, false) + ".pipe(" + strings.Join(parts, ", ") + ")"
}

func togglePipeToFunctionText(sf *ast.SourceFile, subject *ast.Node, args []*ast.Node) string {
	parts := make([]string, 0, len(args)+1)
	parts = append(parts, scanner.GetSourceTextOfNodeFromSourceFile(sf, subject, false))
	for _, arg := range args {
		parts = append(parts, scanner.GetSourceTextOfNodeFromSourceFile(sf, arg, false))
	}
	return "pipe(" + strings.Join(parts, ", ") + ")"
}
