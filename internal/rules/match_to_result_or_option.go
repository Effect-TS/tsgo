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

// MatchToResultOrOption suggests Effect.result, Effect.option or Effect.either
// when match handlers, or adjacent map and catch steps, only reify the outcome.
var MatchToResultOrOption = rule.Rule{
	Name:            "matchToResultOrOption",
	Group:           "style",
	Description:     "Suggests Effect.result, Effect.option or Effect.either when match handlers, or a map step followed by a catch step, only reify the outcome",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Effect_0_reifies_the_outcome_directly_these_Effect_1_handlers_only_wrap_the_value_and_the_error_effect_matchToResultOrOption.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeMatchToResultOrOption(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diagnostics := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diagnostics[i] = ctx.NewDiagnostic(match.SourceFile, match.Location,
				tsdiag.Effect_0_reifies_the_outcome_directly_these_Effect_1_handlers_only_wrap_the_value_and_the_error_effect_matchToResultOrOption,
				nil, match.ReplacementName, match.SourceName)
		}
		return diagnostics
	},
}

type MatchToResultOrOptionMatch struct {
	SourceFile *ast.SourceFile
	Location   core.TextRange
	Flow       *typeparser.PipingFlow
	// Start and End are the inclusive range of steps the replacement covers:
	// the fold step alone, or the map and catch pair.
	Start, End      int
	ReplacementName string // result, option or either
	SourceName      string // match, matchEffect, map/catch or map/catchAll
}

// reifyingKind is the container a constructor wraps into (result, option or
// either) and whether it wraps the failure.
type reifyingKind struct {
	replacement string
	failure     bool
}

var reifyingNone = reifyingKind{replacement: "option", failure: true}

// AnalyzeMatchToResultOrOption finds Effect.match and Effect.matchEffect steps
// whose handlers only wrap the value and the error, and adjacent Effect.map and
// Effect.catch (Effect.catchAll in v3) steps that do the same.
func AnalyzeMatchToResultOrOption(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []MatchToResultOrOptionMatch {
	if tp == nil || c == nil || sf == nil {
		return nil
	}
	isFold := func(step typeparser.PipingFlowTransformation) bool {
		return step.Args().Len() == 1 && (tp.IsNodeReferenceToEffectModuleApi(step.Callee(), "match") ||
			tp.IsNodeReferenceToEffectModuleApi(step.Callee(), "matchEffect"))
	}
	isMap := func(step typeparser.PipingFlowTransformation) bool {
		return step.Args().Len() == 1 && tp.IsNodeReferenceToEffectModuleApi(step.Callee(), "map")
	}
	isCatch := func(step typeparser.PipingFlowTransformation) bool {
		return step.Args().Len() == 1 && matchToResultCatchName(tp, step) != ""
	}

	var matches []MatchToResultOrOptionMatch
	add := func(flow *typeparser.PipingFlow, start, end int, onFailure, onSuccess reifyingKind, sourceName string) {
		if !onFailure.failure || onSuccess.failure || onFailure.replacement != onSuccess.replacement {
			return
		}
		matches = append(matches, MatchToResultOrOptionMatch{
			SourceFile:      sf,
			Location:        scanner.GetErrorRangeForNode(sf, flow.Transformations().At(end).Callee()),
			Flow:            flow,
			Start:           start,
			End:             end,
			ReplacementName: onSuccess.replacement,
			SourceName:      sourceName,
		})
	}
	for _, flow := range tp.PipingFlows(sf, true).All() {
		for _, sequence := range flow.FindTransformationSequences(isFold).All() {
			i := sequence.Start()
			step := flow.Transformations().At(i)
			options := ast.SkipParentheses(step.Args().At(0))
			// Exactly two properties: a spread or any other member could
			// replace or add to the onFailure and onSuccess handlers.
			if options.Kind != ast.KindObjectLiteralExpression || options.AsObjectLiteralExpression().Properties == nil ||
				len(options.AsObjectLiteralExpression().Properties.Nodes) != 2 {
				continue
			}
			classify, sourceName := reifyingHandler, "match"
			if tp.IsNodeReferenceToEffectModuleApi(step.Callee(), "matchEffect") {
				classify, sourceName = succeedReifyingHandler, "matchEffect"
			}
			onFailure, ok := classify(tp, c, typeparser.ObjectLiteralPropertyInitializer(options, "onFailure"))
			if !ok {
				continue
			}
			if onSuccess, ok := classify(tp, c, typeparser.ObjectLiteralPropertyInitializer(options, "onSuccess")); ok {
				add(flow, i, i, onFailure, onSuccess, sourceName)
			}
		}
		for _, sequence := range flow.FindTransformationSequences(isMap, isCatch).All() {
			i := sequence.Start()
			mapStep, catchStep := flow.Transformations().At(i), flow.Transformations().At(i+1)
			onSuccess, ok := reifyingHandler(tp, c, mapStep.Args().At(0))
			if !ok {
				continue
			}
			if onFailure, ok := succeedReifyingHandler(tp, c, catchStep.Args().At(0)); ok {
				add(flow, i, i+1, onFailure, onSuccess, "map/"+matchToResultCatchName(tp, catchStep))
			}
		}
	}
	return matches
}

// reifyingHandler classifies a handler that wraps its argument into a
// container: the constructor itself, an identity forwarder to it, or a
// function whose body is Option.none().
func reifyingHandler(tp *typeparser.TypeParser, _ *checker.Checker, node *ast.Node) (reifyingKind, bool) {
	if node == nil {
		return reifyingKind{}, false
	}
	target, typeArguments, _ := tp.UnwrapIdentityForwarder(node)
	if kind, ok := reifyingConstructor(tp, target); ok {
		return kind, typeArguments == nil || len(typeArguments.Nodes) == 0
	}
	lazy := typeparser.ParseLazyExpression(node, typeparser.LazyExpressionNone)
	return reifyingNone, lazy != nil && hasPlainParameters(lazy) && isReifyingNoneCall(tp, lazy.Expression())
}

// succeedReifyingHandler classifies a handler whose body lifts a reified
// value with Effect.succeed: Effect.succeed(Result.fail(e)) for its parameter
// e, or Effect.succeed(Option.none()), in any flow spelling.
func succeedReifyingHandler(tp *typeparser.TypeParser, c *checker.Checker, node *ast.Node) (reifyingKind, bool) {
	if node == nil {
		return reifyingKind{}, false
	}
	lazy := typeparser.ParseLazyExpression(node, typeparser.LazyExpressionNone)
	if lazy == nil || !hasPlainParameters(lazy) {
		return reifyingKind{}, false
	}
	flow := tp.LongestPipingFlowAt(lazy.Expression(), false)
	isPlainStep := func(step typeparser.PipingFlowTransformation) bool {
		return step.Args().Len() == 0 && !HasExplicitTypeArguments(&step)
	}
	isSucceed := func(step typeparser.PipingFlowTransformation) bool {
		return isPlainStep(step) && tp.IsNodeReferenceToEffectModuleApi(step.Callee(), "succeed")
	}
	if flow.MatchesExactly(func(subject typeparser.PipingFlowSubject) bool {
		return isReifyingNoneCall(tp, subject.Node())
	}, isSucceed) {
		return reifyingNone, true
	}
	var kind reifyingKind
	matched := flow.MatchesExactly(func(subject typeparser.PipingFlowSubject) bool {
		return lazy.Params().Len() == 1 && sameIdentifierReference(tp, c, lazy.Params().At(0).Name(), ast.SkipParentheses(subject.Node()))
	}, func(step typeparser.PipingFlowTransformation) bool {
		var ok bool
		kind, ok = reifyingConstructor(tp, step.Callee())
		return ok && isPlainStep(step) && kind != reifyingNone
	}, isSucceed)
	return kind, matched
}

// reifyingConstructor classifies a reference to a constructor that wraps a
// value or an error into a Result, Option or Either.
func reifyingConstructor(tp *typeparser.TypeParser, node *ast.Node) (reifyingKind, bool) {
	switch {
	case tp.IsNodeReferenceToEffectResultModuleApi(node, "succeed"):
		return reifyingKind{replacement: "result"}, true
	case tp.IsNodeReferenceToEffectResultModuleApi(node, "fail"):
		return reifyingKind{replacement: "result", failure: true}, true
	case tp.IsNodeReferenceToEffectOptionModuleApi(node, "some"):
		return reifyingKind{replacement: "option"}, true
	case tp.IsNodeReferenceToEffectOptionModuleApi(node, "none"):
		return reifyingNone, true
	case tp.IsNodeReferenceToEffectEitherModuleApi(node, "right"):
		return reifyingKind{replacement: "either"}, true
	case tp.IsNodeReferenceToEffectEitherModuleApi(node, "left"):
		return reifyingKind{replacement: "either", failure: true}, true
	}
	return reifyingKind{}, false
}

// isReifyingNoneCall reports whether node is Option.none(), with or without
// explicit type arguments.
func isReifyingNoneCall(tp *typeparser.TypeParser, node *ast.Node) bool {
	node = ast.SkipParentheses(node)
	if node == nil || node.Kind != ast.KindCallExpression {
		return false
	}
	call := node.AsCallExpression()
	if call.Arguments != nil && len(call.Arguments.Nodes) > 0 {
		return false
	}
	kind, ok := reifyingConstructor(tp, call.Expression)
	return ok && kind == reifyingNone
}

// hasPlainParameters reports whether a handler declares at most one
// parameter, bound to an identifier without a default or rest marker.
func hasPlainParameters(lazy *typeparser.ParsedLazyExpression) bool {
	if lazy.Params().Len() > 1 {
		return false
	}
	for _, parameter := range lazy.Params().All() {
		declaration := parameter.AsParameterDeclaration()
		if declaration.Name() == nil || declaration.Name().Kind != ast.KindIdentifier ||
			declaration.DotDotDotToken != nil || declaration.Initializer != nil {
			return false
		}
	}
	return true
}

// matchToResultCatchName returns catch or catchAll for an Effect catch step,
// and "" for any other step.
func matchToResultCatchName(tp *typeparser.TypeParser, step typeparser.PipingFlowTransformation) string {
	for _, name := range [...]string{"catch", "catchAll"} {
		if tp.IsNodeReferenceToEffectModuleApi(step.Callee(), name) {
			return name
		}
	}
	return ""
}
