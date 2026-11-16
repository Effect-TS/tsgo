package fixables

import (
	"strconv"

	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var CatchTagToCatchReasonFix = fixable.Fixable{
	Name:        "catchTagToCatchReason",
	Description: "Replace hand-rolled reason dispatch with Effect.catchReason or Effect.catchReasons",
	ErrorCodes: []int32{
		tsdiag.Branching_on_0_reason_tag_inside_Effect_1_hand_rolls_reason_dispatch_use_Effect_catchReason_or_Effect_catchReasons_which_re_fail_unmatched_reasons_automatically_effect_catchTagToCatchReason.Code(),
	},
	FixIDs: []string{"catchTagToCatchReason_fix"},
	Run:    runCatchTagToCatchReasonFix,
}

func runCatchTagToCatchReasonFix(ctx *fixable.Context) []ls.CodeAction {
	matches := rules.AnalyzeCatchTagToCatchReason(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
	for _, match := range matches {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		// Only Effect.catchTag is rewritten. The new handlers rebind the parameter
		// per branch, which only holds for an arrow function whose parameter is
		// never reassigned.
		if match.Transformation == nil || match.Callee == nil || match.OuterTag == nil ||
			match.Handler == nil || match.Handler.Kind != ast.KindArrowFunction ||
			checker.Checker_isSymbolAssigned(ctx.Checker, match.ParameterSymbol) {
			continue
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect.catchReason or Effect.catchReasons",
			Run: func(tracker *rewriter.Tracker) {
				callee, arguments := buildCatchTagToCatchReasonReplacement(tracker, ctx, match)
				if callee == nil || arguments == nil {
					return
				}
				tracker.ReplacePipingFlowTransformation(ctx.SourceFile, match.Transformation, rewriter.PipingFlowTransformationReplacement{
					Callee:    callee,
					Arguments: arguments,
				})
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}

	return nil
}

func buildCatchTagToCatchReasonReplacement(tracker *rewriter.Tracker, ctx *fixable.Context, match rules.CatchTagToCatchReasonMatch) (*ast.Node, *ast.NodeList) {
	if tracker == nil || match.Callee == nil || match.OuterTag == nil || len(match.Branches) == 0 {
		return nil, nil
	}
	methodName := "catchReason"
	if len(match.Branches) > 1 {
		methodName = "catchReasons"
	}
	method := tracker.NewPropertyAccessPath(typeparser.FindEffectModuleReference(ctx.SourceFile).Append(methodName)...)
	arguments := []*ast.Node{tracker.DeepCloneNode(match.OuterTag)}
	if len(match.Branches) == 1 {
		branch := match.Branches[0]
		arguments = append(arguments, tracker.NewStringLiteral(branch.ReasonTag, 0), newCatchReasonHandler(tracker, ctx.Checker, match.ParameterName, branch))
	} else {
		properties := make([]*ast.Node, 0, len(match.Branches))
		for _, branch := range match.Branches {
			properties = append(properties, tracker.NewPropertyAssignment(
				nil,
				tracker.NewStringLiteral(branch.ReasonTag, 0),
				nil,
				nil,
				newCatchReasonHandler(tracker, ctx.Checker, match.ParameterName, branch),
			))
		}
		arguments = append(arguments, tracker.NewObjectLiteralExpression(tracker.NewNodeList(properties), false))
	}

	return method, tracker.NewNodeList(arguments)
}

func newCatchReasonHandler(tracker *rewriter.Tracker, c *checker.Checker, parameterName string, branch rules.CatchTagToCatchReasonBranch) *ast.Node {
	var parameters []*ast.Node
	if branch.UsesParameter {
		parameters = []*ast.Node{
			tracker.NewParameterDeclaration(nil, nil, tracker.NewIdentifier(uniqueCatchReasonParameterName(c, branch.Result)), nil, nil, nil),
			tracker.NewParameterDeclaration(nil, nil, tracker.NewIdentifier(parameterName), nil, nil, nil),
		}
	}
	return tracker.NewArrowFunction(
		nil,
		nil,
		tracker.NewNodeList(parameters),
		nil,
		nil,
		tracker.NewToken(ast.KindEqualsGreaterThanToken),
		tracker.DeepCloneNode(branch.Result),
	)
}

// uniqueCatchReasonParameterName returns "_", or "_0", "_1", ... when taken,
// for the unused reason parameter that precedes the error parameter.
func uniqueCatchReasonParameterName(c *checker.Checker, location *ast.Node) string {
	used := make(map[string]struct{})
	for _, symbol := range c.GetSymbolsInScope(location, ast.SymbolFlagsValue) {
		used[c.SymbolToString(symbol)] = struct{}{}
	}
	for i := -1; ; i++ {
		name := "_"
		if i >= 0 {
			name = "_" + strconv.Itoa(i)
		}
		if _, exists := used[name]; !exists {
			return name
		}
	}
}
