package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

// PreferSucceedSomeOrNone suggests Effect.succeedNone and Effect.succeedSome for
// Effect.succeed calls that directly wrap Option.none or Option.some.
var PreferSucceedSomeOrNone = rule.Rule{
	Name:            "preferSucceedSomeOrNone",
	Group:           "style",
	Description:     "Suggests using Effect.succeedNone or Effect.succeedSome instead of wrapping Option.none or Option.some with Effect.succeed",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Effect_0_expresses_this_Option_success_value_directly_effect_preferSucceedSomeOrNone.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzePreferSucceedSomeOrNone(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diagnostics := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diagnostics[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.Effect_0_expresses_this_Option_success_value_directly_effect_preferSucceedSomeOrNone,
				nil,
				match.ReplacementName,
			)
		}
		return diagnostics
	},
}

// PreferSucceedSomeOrNoneMatch holds the nodes needed by the diagnostic and quick fix.
type PreferSucceedSomeOrNoneMatch struct {
	SourceFile         *ast.SourceFile
	Location           core.TextRange
	ReplacementTarget  *ast.Node
	EffectModuleNode   *ast.Node
	ReplacementName    string
	ValueNode          *ast.Node
	ValueTypeArguments *ast.NodeList
}

type normalizedOptionInput struct {
	ReplacementName    string
	ValueNode          *ast.Node
	ValueTypeArguments *ast.NodeList
}

// AnalyzePreferSucceedSomeOrNone finds piping flows in which Option.none or
// Option.some feeds directly into Effect.succeed. This covers both nested calls
// and pipe forms such as Option.none().pipe(Effect.succeed).
func AnalyzePreferSucceedSomeOrNone(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile) []PreferSucceedSomeOrNoneMatch {
	if tp == nil || sf == nil {
		return nil
	}

	var matches []PreferSucceedSomeOrNoneMatch
	seen := make(map[*ast.Node]struct{})
	for _, flow := range tp.PipingFlows(sf, true) {
		for index := range flow.Transformations {
			transformation := &flow.Transformations[index]
			if transformation.Callee == nil || transformation.Callee.Kind != ast.KindPropertyAccessExpression ||
				len(transformation.Args) != 0 ||
				!tp.IsNodeReferenceToEffectModuleApi(transformation.Callee, "succeed") {
				continue
			}
			if _, ok := seen[transformation.Node]; ok {
				continue
			}

			if transformation.Node != nil && transformation.Node.Kind == ast.KindCallExpression {
				call := transformation.Node.AsCallExpression()
				if call != nil &&
					call.TypeArguments != nil && len(call.TypeArguments.Nodes) > 0 {
					continue
				}
			}

			optionInput := matchNormalizedOptionInput(tp, flow, index)
			if optionInput == nil {
				continue
			}

			match := PreferSucceedSomeOrNoneMatch{
				SourceFile:         sf,
				Location:           scanner.GetErrorRangeForNode(sf, transformation.Callee),
				EffectModuleNode:   transformation.Callee.AsPropertyAccessExpression().Expression,
				ReplacementName:    optionInput.ReplacementName,
				ValueNode:          optionInput.ValueNode,
				ValueTypeArguments: optionInput.ValueTypeArguments,
			}

			// A direct Effect.succeed(...) call can be replaced in place even when
			// more transformations follow. A pipe spelling is safely auto-fixable
			// when the matched Option -> succeed pair is the complete flow.
			if transformation.Node != nil && transformation.Node.Kind == ast.KindCallExpression {
				match.ReplacementTarget = transformation.Node
			} else if index == len(flow.Transformations)-1 &&
				(match.ReplacementName == "succeedNone" && index == 0 ||
					match.ReplacementName == "succeedSome" && index == 1) {
				match.ReplacementTarget = flow.Node
			}

			seen[transformation.Node] = struct{}{}
			matches = append(matches, match)
		}
	}
	return matches
}

func matchNormalizedOptionInput(tp *typeparser.TypeParser, flow *typeparser.PipingFlow, succeedIndex int) *normalizedOptionInput {
	if succeedIndex == 0 {
		subject := flow.Subject.Node
		if subject == nil || subject.Kind != ast.KindCallExpression {
			return nil
		}
		call := subject.AsCallExpression()
		if call == nil || call.Arguments == nil || len(call.Arguments.Nodes) != 0 ||
			call.TypeArguments != nil && len(call.TypeArguments.Nodes) > 0 ||
			!tp.IsNodeReferenceToEffectOptionModuleApi(call.Expression, "none") {
			return nil
		}
		return &normalizedOptionInput{ReplacementName: "succeedNone"}
	}

	previous := &flow.Transformations[succeedIndex-1]
	if previous.Callee == nil || len(previous.Args) != 0 ||
		!tp.IsNodeReferenceToEffectOptionModuleApi(previous.Callee, "some") {
		return nil
	}

	input := &normalizedOptionInput{ReplacementName: "succeedSome"}
	if previous.Node != nil && previous.Node.Kind == ast.KindCallExpression {
		call := previous.Node.AsCallExpression()
		if call == nil || call.Arguments == nil || len(call.Arguments.Nodes) != 1 {
			return nil
		}
		input.ValueNode = call.Arguments.Nodes[0]
		input.ValueTypeArguments = call.TypeArguments
	} else if succeedIndex == 1 {
		input.ValueNode = flow.Subject.Node
	}
	return input
}
