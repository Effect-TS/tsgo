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
	SourceFile          *ast.SourceFile
	Location            core.TextRange
	Flow                *typeparser.PipingFlow
	TransformationCount int
	EffectModuleNode    *ast.Node
	ReplacementName     string
	ValueNode           *ast.Node
	ValueTypeArguments  *ast.NodeList
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
	for _, flow := range tp.PipingFlows(sf, true) {
		isSucceed := func(transformation *typeparser.PipingFlowTransformation) bool {
			return transformation.Callee != nil &&
				len(transformation.Args) == 0 &&
				(transformation.TypeArguments == nil || len(transformation.TypeArguments.Nodes) == 0) &&
				tp.IsNodeReferenceToEffectModuleApi(transformation.Callee, "succeed")
		}
		if flow.MatchesPrefix(
			func(subject *typeparser.PipingFlowSubject) bool { return isOptionNoneCall(tp, subject.Node) },
			isSucceed,
		) {
			matches = append(matches, preferSucceedSomeOrNoneMatch(sf, flow, 0, &normalizedOptionInput{ReplacementName: "succeedNone"}))
		}

		sequences := flow.FindTransformationSequences(
			func(transformation *typeparser.PipingFlowTransformation) bool {
				return transformation.Callee != nil && len(transformation.Args) == 0 &&
					tp.IsNodeReferenceToEffectOptionModuleApi(transformation.Callee, "some")
			},
			isSucceed,
		)
		for _, sequence := range sequences {
			optionIndex := sequence.Start
			matches = append(matches, preferSucceedSomeOrNoneMatch(sf, flow, optionIndex+1, &normalizedOptionInput{
				ReplacementName:    "succeedSome",
				ValueNode:          flow.TransformationInputNode(optionIndex),
				ValueTypeArguments: flow.Transformations[optionIndex].TypeArguments,
			}))
		}
	}
	return matches
}

func preferSucceedSomeOrNoneMatch(sf *ast.SourceFile, flow *typeparser.PipingFlow, succeedIndex int, optionInput *normalizedOptionInput) PreferSucceedSomeOrNoneMatch {
	transformation := &flow.Transformations[succeedIndex]
	var effectModuleNode *ast.Node
	if transformation.Callee.Kind == ast.KindPropertyAccessExpression {
		effectModuleNode = transformation.Callee.AsPropertyAccessExpression().Expression
	}
	match := PreferSucceedSomeOrNoneMatch{
		SourceFile:         sf,
		Location:           scanner.GetErrorRangeForNode(sf, transformation.Callee),
		EffectModuleNode:   effectModuleNode,
		ReplacementName:    optionInput.ReplacementName,
		ValueNode:          optionInput.ValueNode,
		ValueTypeArguments: optionInput.ValueTypeArguments,
	}
	if transformation.Kind == typeparser.TransformationKindCall ||
		transformation.Kind == typeparser.TransformationKindPipe ||
		transformation.Kind == typeparser.TransformationKindPipeable {
		match.Flow = flow
		match.TransformationCount = succeedIndex + 1
	}
	return match
}

func isOptionNoneCall(tp *typeparser.TypeParser, node *ast.Node) bool {
	if node == nil || node.Kind != ast.KindCallExpression {
		return false
	}
	call := node.AsCallExpression()
	return call != nil && call.Arguments != nil && len(call.Arguments.Nodes) == 0 &&
		(call.TypeArguments == nil || len(call.TypeArguments.Nodes) == 0) &&
		tp.IsNodeReferenceToEffectOptionModuleApi(call.Expression, "none")
}
