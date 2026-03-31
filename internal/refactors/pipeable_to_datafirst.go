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

var PipeableToDatafirst = refactor.Refactor{
	Name:        "pipeableToDatafirst",
	Description: "Rewrite to datafirst",
	Kind:        "rewrite.effect.pipeableToDatafirst",
	Run:         runPipeableToDatafirst,
}

func runPipeableToDatafirst(ctx *refactor.Context) []ls.CodeAction {
	c := ctx.Checker

	token := astnav.GetTokenAtPosition(ctx.SourceFile, ctx.Span.Pos())
	if token == nil {
		return nil
	}

	// Walk up the ancestor chain looking for a pipe(...) call
	for node := token; node != nil; node = node.Parent {
		if node.Kind != ast.KindCallExpression {
			continue
		}

		pipeCall := typeparser.ParsePipeCall(c, node)
		if pipeCall == nil {
			continue
		}

		// Only applies to pipe() calls (not .pipe())
		if pipeCall.Kind != typeparser.TransformationKindPipe {
			continue
		}

		// The pipe identifier must be within the selection range
		callExpr := node.AsCallExpression()
		if callExpr == nil || callExpr.Expression == nil {
			continue
		}
		exprPos := callExpr.Expression.Pos()
		exprEnd := callExpr.Expression.End()
		if exprPos > ctx.Span.End() || exprEnd < ctx.Span.Pos() {
			continue
		}

		// Need at least one argument after the subject
		if len(pipeCall.Args) == 0 {
			continue
		}

		// Try converting pipe args to data-first style
		result := tryConvertToDatafirst(c, pipeCall, ctx, node)
		if result == nil {
			continue
		}

		return result
	}

	return nil
}

func tryConvertToDatafirst(c *checker.Checker, pipeCall *typeparser.ParsedPipeCallResult, ctx *refactor.Context, node *ast.Node) []ls.CodeAction {
	// Accumulate: start with the subject, then process each pipe argument
	// Track whether we successfully converted at least one step
	type conversionStep struct {
		isDataFirst bool
		arg         *ast.Node // original arg node
	}

	steps := make([]conversionStep, len(pipeCall.Args))
	didSomething := false

	for i, arg := range pipeCall.Args {
		if arg.Kind == ast.KindCallExpression {
			argCall := arg.AsCallExpression()
			if argCall != nil && argCall.Expression != nil {
				exprType := typeparser.GetTypeAtLocation(c, argCall.Expression)
				if exprType != nil {
					callSigs := c.GetSignaturesOfType(exprType, checker.SignatureKindCall)
					argCount := 0
					if argCall.Arguments != nil {
						argCount = len(argCall.Arguments.Nodes)
					}
					for _, sig := range callSigs {
						if len(sig.Parameters()) == argCount+1 {
							steps[i] = conversionStep{isDataFirst: true, arg: arg}
							didSomething = true
							break
						}
					}
					if !steps[i].isDataFirst {
						steps[i] = conversionStep{isDataFirst: false, arg: arg}
					}
					continue
				}
			}
		}
		steps[i] = conversionStep{isDataFirst: false, arg: arg}
	}

	if !didSomething {
		return nil
	}

	action := ctx.NewRefactorAction(refactor.RefactorAction{
		Description: "Rewrite to datafirst",
		Run: func(tracker *change.Tracker) {
			// Build the replacement node by accumulating from the subject
			newNode := tracker.DeepCloneNode(pipeCall.Subject)

			for _, step := range steps {
				if step.isDataFirst {
					// Convert: callee(args...) + self -> callee(self, args...)
					argCall := step.arg.AsCallExpression()
					clonedCallee := tracker.DeepCloneNode(argCall.Expression)

					allArgs := make([]*ast.Node, 0, 1+len(argCall.Arguments.Nodes))
					allArgs = append(allArgs, newNode)
					if argCall.Arguments != nil {
						for _, a := range argCall.Arguments.Nodes {
							allArgs = append(allArgs, tracker.DeepCloneNode(a))
						}
					}

					callExpr := tracker.NewCallExpression(clonedCallee, nil, nil, tracker.NewNodeList(allArgs), ast.NodeFlagsNone)
					ast.SetParentInChildren(callExpr)
					newNode = callExpr
				} else {
					// Not data-first: keep in pipe form
					// If newNode is already a pipe call, extend it
					if isPipeCallNode(newNode) {
						pipeExprCall := newNode.AsCallExpression()
						existingArgs := make([]*ast.Node, 0)
						if pipeExprCall.Arguments != nil {
							existingArgs = append(existingArgs, pipeExprCall.Arguments.Nodes...)
						}
						existingArgs = append(existingArgs, tracker.DeepCloneNode(step.arg))

						pipeId := tracker.NewIdentifier("pipe")
						callExpr := tracker.NewCallExpression(pipeId, nil, nil, tracker.NewNodeList(existingArgs), ast.NodeFlagsNone)
						ast.SetParentInChildren(callExpr)
						newNode = callExpr
					} else {
						// Wrap in pipe(self, arg)
						pipeId := tracker.NewIdentifier("pipe")
						pipeArgs := []*ast.Node{newNode, tracker.DeepCloneNode(step.arg)}
						callExpr := tracker.NewCallExpression(pipeId, nil, nil, tracker.NewNodeList(pipeArgs), ast.NodeFlagsNone)
						ast.SetParentInChildren(callExpr)
						newNode = callExpr
					}
				}
			}

			tracker.ReplaceNode(ctx.SourceFile, node, newNode, nil)
		},
	})

	if action == nil {
		return nil
	}
	action.Kind = "refactor.rewrite.effect.pipeableToDatafirst"
	return []ls.CodeAction{*action}
}

// isPipeCallNode checks if a synthesized node is a pipe(...) call by checking if the
// expression is an identifier named "pipe".
func isPipeCallNode(node *ast.Node) bool {
	if node == nil || node.Kind != ast.KindCallExpression {
		return false
	}
	call := node.AsCallExpression()
	if call == nil || call.Expression == nil {
		return false
	}
	if call.Expression.Kind != ast.KindIdentifier {
		return false
	}
	return call.Expression.AsIdentifier() != nil && call.Expression.AsIdentifier().Text == "pipe"
}
