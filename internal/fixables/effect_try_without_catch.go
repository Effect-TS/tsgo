package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

var EffectTryWithoutCatchFix = fixable.Fixable{
	Name:        "effectTryWithoutCatch",
	Description: "Add a catch handler (object form)",
	ErrorCodes: []int32{
		tsdiag.Effect_0_is_called_without_a_catch_handler_so_failures_surface_as_the_generic_1_The_object_form_try_catch_maps_the_cause_to_a_typed_domain_specific_error_effect_effectTryWithoutCatch.Code(),
	},
	FixIDs: []string{"effectTryWithoutCatch_fix"},
	Run:    runEffectTryWithoutCatchFix,
}

func runEffectTryWithoutCatchFix(ctx *fixable.Context) []ls.CodeAction {
	sf := ctx.SourceFile
	for _, match := range rules.AnalyzeEffectTryWithoutCatch(ctx.TypeParser, ctx.Checker, sf) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		// The thunk moves into the options object, so it must be a standalone
		// node: a later pipe step has no node for its input.
		if match.Thunk == nil {
			continue
		}
		step := match.Step()
		// The short form takes <A>, the object form <A, E>: a single written type
		// argument gets an `unknown` error type, while two cannot be the short form.
		typeArguments := step.TypeArguments()
		if typeArguments != nil && len(typeArguments.Nodes) > 1 {
			continue
		}
		if !calleeHasEffectApiType(ctx.TypeParser, ctx.Checker, step.Callee()) {
			continue
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Add a catch handler (object form)",
			Run: func(tracker *rewriter.Tracker) {
				if step.Kind() == typeparser.TransformationKindCall {
					// The thunk is already the call's argument: wrapping it in place keeps
					// its text and comments, which the flow edits would reprint.
					if typeArguments != nil && len(typeArguments.Nodes) == 1 {
						tracker.InsertText(sf, ctx.BytePosToLSPPosition(typeArguments.Nodes[0].End()), ", unknown")
					}
					tracker.InsertText(sf, ctx.BytePosToLSPPosition(scanner.GetTokenPosOfNode(match.Thunk, sf, false)), "{ try: ")
					tracker.InsertText(sf, ctx.BytePosToLSPPosition(match.Thunk.End()), ", catch: (cause) => cause }")
					return
				}

				// A pipe step applies the API to its subject, so the subject and the
				// step become one object-form call, keeping the later steps.
				var typeArgumentList *ast.NodeList
				if typeArguments != nil && len(typeArguments.Nodes) == 1 {
					typeArgumentList = tracker.NewNodeList([]*ast.Node{
						tracker.DeepCloneNode(typeArguments.Nodes[0]),
						tracker.NewKeywordTypeNode(ast.KindUnknownKeyword),
					})
				}
				cause := tracker.NewIdentifier("cause")
				catchHandler := tracker.NewArrowFunction(
					nil, nil,
					tracker.NewNodeList([]*ast.Node{tracker.NewParameterDeclaration(nil, nil, cause, nil, nil, nil)}),
					nil, nil,
					tracker.NewToken(ast.KindEqualsGreaterThanToken),
					tracker.NewIdentifier("cause"),
				)
				options := tracker.NewObjectLiteralExpression(tracker.NewNodeList([]*ast.Node{
					tracker.NewPropertyAssignment(nil, tracker.NewIdentifier("try"), nil, nil, tracker.DeepCloneNode(match.Thunk)),
					tracker.NewPropertyAssignment(nil, tracker.NewIdentifier("catch"), nil, nil, catchHandler),
				}), false)
				replacement := tracker.NewCallExpression(
					tracker.DeepCloneNode(step.Callee()),
					nil,
					typeArgumentList,
					tracker.NewNodeList([]*ast.Node{options}),
					ast.NodeFlagsNone,
				)
				tracker.ReplacePipingFlowPrefix(sf, match.Flow, match.Index+1, replacement)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}

	return nil
}

// calleeHasEffectApiType reports whether callee still has the declared type of
// the Effect API it resolves to, and so its object-form overload. Const aliases
// resolve to the API whatever their annotation, which may drop that overload.
func calleeHasEffectApiType(tp *typeparser.TypeParser, c *checker.Checker, callee *ast.Node) bool {
	symbol := tp.ReferenceSymbolAtNode(callee)
	calleeType := tp.GetTypeAtLocation(callee)
	return symbol != nil && calleeType != nil && c.GetTypeOfSymbol(symbol) == calleeType
}
