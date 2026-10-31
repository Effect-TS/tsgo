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
	Description:     "Suggests Effect.result, Effect.option or Effect.either when match handlers only reify the outcome",
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
	Fixable         bool
}

// reifyingFamily is the container a handler reifies the outcome into.
type reifyingFamily uint8

const (
	reifyingResult reifyingFamily = iota
	reifyingOption
	reifyingEither
)

var reifyingReplacementNames = [...]string{
	reifyingResult: "result",
	reifyingOption: "option",
	reifyingEither: "either",
}

// reifyingKind is a constructor's container and whether it wraps the failure.
type reifyingKind struct {
	family  reifyingFamily
	failure bool
}

// AnalyzeMatchToResultOrOption finds Effect.match and Effect.matchEffect steps
// whose handlers only wrap the value and the error, and adjacent Effect.map and
// Effect.catch (Effect.catchAll in v3) steps that do the same.
func AnalyzeMatchToResultOrOption(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []MatchToResultOrOptionMatch {
	if tp == nil || c == nil || sf == nil {
		return nil
	}
	isMap := func(step typeparser.PipingFlowTransformation) bool {
		return hasSingleArgument(step) && tp.IsNodeReferenceToEffectModuleApi(step.Callee(), "map")
	}
	isCatch := func(step typeparser.PipingFlowTransformation) bool {
		_, ok := matchToResultCatchName(tp, step)
		return ok
	}

	var matches []MatchToResultOrOptionMatch
	for _, flow := range tp.PipingFlows(sf, true).All() {
		for i, step := range flow.Transformations().All() {
			kind, sourceName, ok := analyzeReifyingFold(tp, c, step)
			if !ok {
				continue
			}
			matches = append(matches, newMatchToResultOrOptionMatch(tp, c, sf, flow, i, i, kind, sourceName))
		}
		for _, sequence := range flow.FindTransformationSequences(isMap, isCatch).All() {
			i := sequence.Start()
			mapStep, catchStep := flow.Transformations().At(i), flow.Transformations().At(i+1)
			success, ok := reifyingHandler(tp, c, mapStep.Args().At(0))
			if !ok || success.failure {
				continue
			}
			failure, ok := succeedReifyingHandler(tp, c, catchStep.Args().At(0))
			if !ok || !failure.failure || failure.family != success.family {
				continue
			}
			catchName, _ := matchToResultCatchName(tp, catchStep)
			matches = append(matches, newMatchToResultOrOptionMatch(tp, c, sf, flow, i, i+1, success, "map/"+catchName))
		}
	}
	return matches
}

func newMatchToResultOrOptionMatch(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile, flow *typeparser.PipingFlow, start, end int, kind reifyingKind, sourceName string) MatchToResultOrOptionMatch {
	return MatchToResultOrOptionMatch{
		SourceFile:      sf,
		Location:        scanner.GetErrorRangeForNode(sf, flow.Transformations().At(end).Callee()),
		Flow:            flow,
		Start:           start,
		End:             end,
		ReplacementName: reifyingReplacementNames[kind.family],
		SourceName:      sourceName,
		Fixable:         matchToResultOrOptionFixable(tp, c, flow, start, end),
	}
}

// analyzeReifyingFold classifies an Effect.match or Effect.matchEffect step
// whose options are exactly an onFailure and an onSuccess handler that wrap
// the error and the value into the same container.
func analyzeReifyingFold(tp *typeparser.TypeParser, c *checker.Checker, step typeparser.PipingFlowTransformation) (reifyingKind, string, bool) {
	if !hasSingleArgument(step) {
		return reifyingKind{}, "", false
	}
	onFailure, onSuccess := matchHandlerPair(step.Args().At(0))
	if onFailure == nil || onSuccess == nil {
		return reifyingKind{}, "", false
	}
	var classify func(*ast.Node) (reifyingKind, bool)
	var sourceName string
	switch {
	case tp.IsNodeReferenceToEffectModuleApi(step.Callee(), "match"):
		classify = func(node *ast.Node) (reifyingKind, bool) { return reifyingHandler(tp, c, node) }
		sourceName = "match"
	case tp.IsNodeReferenceToEffectModuleApi(step.Callee(), "matchEffect"):
		classify = func(node *ast.Node) (reifyingKind, bool) { return succeedReifyingHandler(tp, c, node) }
		sourceName = "matchEffect"
	default:
		return reifyingKind{}, "", false
	}
	failure, ok := classify(onFailure)
	if !ok || !failure.failure {
		return reifyingKind{}, "", false
	}
	success, ok := classify(onSuccess)
	if !ok || success.failure || success.family != failure.family {
		return reifyingKind{}, "", false
	}
	return success, sourceName, true
}

// matchHandlerPair returns the handlers of an object literal whose only
// properties are onFailure and onSuccess assignments.
func matchHandlerPair(node *ast.Node) (onFailure, onSuccess *ast.Node) {
	node = ast.SkipParentheses(node)
	if node == nil || node.Kind != ast.KindObjectLiteralExpression {
		return nil, nil
	}
	properties := node.AsObjectLiteralExpression().Properties
	if properties == nil || len(properties.Nodes) != 2 {
		return nil, nil
	}
	for _, property := range properties.Nodes {
		if property.Kind != ast.KindPropertyAssignment {
			return nil, nil
		}
		name := property.Name()
		if name == nil || name.Kind != ast.KindIdentifier && name.Kind != ast.KindStringLiteral {
			return nil, nil
		}
		switch name.Text() {
		case "onFailure":
			onFailure = property.Initializer()
		case "onSuccess":
			onSuccess = property.Initializer()
		}
	}
	return onFailure, onSuccess
}

// reifyingHandler classifies a handler that wraps its argument into a
// container: the constructor itself, a forwarder to it, or a function whose
// body is exactly Option.none().
func reifyingHandler(tp *typeparser.TypeParser, c *checker.Checker, node *ast.Node) (reifyingKind, bool) {
	target, typeArguments, parameter := tp.UnwrapIdentityForwarder(node)
	if typeArguments != nil && len(typeArguments.Nodes) > 0 {
		return reifyingKind{}, false
	}
	if kind, ok := reifyingConstructor(tp, c, target); ok {
		// Option.none takes no argument, so it is never forwarded one.
		if parameter != nil && kind == (reifyingKind{family: reifyingOption, failure: true}) {
			return reifyingKind{}, false
		}
		return kind, true
	}
	lazy := typeparser.ParseLazyExpression(node, typeparser.LazyExpressionNone)
	if lazy != nil && hasPlainParameters(lazy) && isReifyingNoneCall(tp, c, lazy.Expression()) {
		return reifyingKind{family: reifyingOption, failure: true}, true
	}
	return reifyingKind{}, false
}

// succeedReifyingHandler classifies a handler whose body lifts a reified
// argument with Effect.succeed: Effect.succeed(Result.fail(e)) for its
// parameter e, or Effect.succeed(Option.none()). Only direct calls count: a
// pipe call in the body may run code through its receiver.
func succeedReifyingHandler(tp *typeparser.TypeParser, c *checker.Checker, node *ast.Node) (reifyingKind, bool) {
	lazy := typeparser.ParseLazyExpression(node, typeparser.LazyExpressionNone)
	if lazy == nil || !hasPlainParameters(lazy) {
		return reifyingKind{}, false
	}
	flow := tp.LongestPipingFlowAt(lazy.Expression(), false)
	isSucceed := func(step typeparser.PipingFlowTransformation) bool {
		callee := ast.SkipParentheses(step.Callee())
		return isDirectReifyingCall(step) && isSideEffectFreeReference(tp, c, callee) &&
			tp.IsNodeReferenceToEffectModuleApi(callee, "succeed")
	}
	if flow.MatchesExactly(func(subject typeparser.PipingFlowSubject) bool {
		return isReifyingNoneCall(tp, c, subject.Node())
	}, isSucceed) {
		return reifyingKind{family: reifyingOption, failure: true}, true
	}
	var kind reifyingKind
	matched := flow.MatchesExactly(func(subject typeparser.PipingFlowSubject) bool {
		return lazy.Params().Len() == 1 && sameIdentifierReference(tp, c, lazy.Params().At(0).Name(), ast.SkipParentheses(subject.Node()))
	}, func(step typeparser.PipingFlowTransformation) bool {
		if !isDirectReifyingCall(step) {
			return false
		}
		var ok bool
		kind, ok = reifyingConstructor(tp, c, step.Callee())
		return ok && kind != (reifyingKind{family: reifyingOption, failure: true})
	}, isSucceed)
	return kind, matched
}

// isDirectReifyingCall reports whether a handler body step is a plain call
// such as Effect.succeed(x), without explicit type arguments.
func isDirectReifyingCall(step typeparser.PipingFlowTransformation) bool {
	return step.Kind() == typeparser.TransformationKindCall && step.Args().Len() == 0 && !hasExplicitTypeArguments(&step)
}

// isSideEffectFreeReference reports whether evaluating node runs no code, so
// dropping the handler or step that holds it changes nothing: an identifier,
// or a property read through module namespaces only. Other receivers may run
// code while resolving to the same export, such as the call in
// getOption().none or a getter in modules.option.none.
func isSideEffectFreeReference(tp *typeparser.TypeParser, c *checker.Checker, node *ast.Node) bool {
	if node == nil {
		return false
	}
	switch node.Kind {
	case ast.KindIdentifier:
		return true
	case ast.KindPropertyAccessExpression:
		access := node.AsPropertyAccessExpression()
		return access.QuestionDotToken == nil && isModuleNamespaceReference(tp, c, ast.SkipParentheses(access.Expression))
	}
	return false
}

// isModuleNamespaceReference reports whether node is a side-effect-free
// reference to a module namespace, such as an import of effect/Option.
func isModuleNamespaceReference(tp *typeparser.TypeParser, c *checker.Checker, node *ast.Node) bool {
	if !isSideEffectFreeReference(tp, c, node) {
		return false
	}
	symbol := tp.GetSymbolAtLocation(node)
	if symbol != nil && ast.TSGetSymbolFlags(symbol)&ast.SymbolFlagsAlias != 0 {
		symbol = c.GetAliasedSymbol(symbol)
	}
	return symbol != nil && ast.TSGetSymbolFlags(symbol)&ast.SymbolFlagsValueModule != 0
}

// reifyingConstructor classifies a side-effect-free reference to a
// constructor that wraps a value or an error into a Result, Option or Either.
func reifyingConstructor(tp *typeparser.TypeParser, c *checker.Checker, node *ast.Node) (reifyingKind, bool) {
	node = ast.SkipParentheses(node)
	if !isSideEffectFreeReference(tp, c, node) {
		return reifyingKind{}, false
	}
	switch {
	case tp.IsNodeReferenceToEffectResultModuleApi(node, "succeed"):
		return reifyingKind{family: reifyingResult}, true
	case tp.IsNodeReferenceToEffectResultModuleApi(node, "fail"):
		return reifyingKind{family: reifyingResult, failure: true}, true
	case tp.IsNodeReferenceToEffectOptionModuleApi(node, "some"):
		return reifyingKind{family: reifyingOption}, true
	case tp.IsNodeReferenceToEffectOptionModuleApi(node, "none"):
		return reifyingKind{family: reifyingOption, failure: true}, true
	case tp.IsNodeReferenceToEffectEitherModuleApi(node, "right"):
		return reifyingKind{family: reifyingEither}, true
	case tp.IsNodeReferenceToEffectEitherModuleApi(node, "left"):
		return reifyingKind{family: reifyingEither, failure: true}, true
	}
	return reifyingKind{}, false
}

// isReifyingNoneCall reports whether node is Option.none(), with or without
// explicit type arguments.
func isReifyingNoneCall(tp *typeparser.TypeParser, c *checker.Checker, node *ast.Node) bool {
	node = ast.SkipParentheses(node)
	if node == nil || node.Kind != ast.KindCallExpression {
		return false
	}
	call := node.AsCallExpression()
	if call.Arguments != nil && len(call.Arguments.Nodes) > 0 {
		return false
	}
	kind, ok := reifyingConstructor(tp, c, call.Expression)
	return ok && kind == reifyingKind{family: reifyingOption, failure: true}
}

// hasPlainParameters reports whether a handler declares at most one
// parameter, bound to an identifier without a default or rest marker.
func hasPlainParameters(lazy *typeparser.ParsedLazyExpression) bool {
	if lazy.Params().Len() > 1 {
		return false
	}
	for _, parameter := range lazy.Params().All() {
		if parameter.Kind != ast.KindParameter {
			return false
		}
		declaration := parameter.AsParameterDeclaration()
		if declaration.Name() == nil || declaration.Name().Kind != ast.KindIdentifier ||
			declaration.DotDotDotToken != nil || declaration.Initializer != nil {
			return false
		}
	}
	return true
}

func matchToResultCatchName(tp *typeparser.TypeParser, step typeparser.PipingFlowTransformation) (string, bool) {
	if !hasSingleArgument(step) || !isFunctionLiteral(step.Args().At(0)) {
		return "", false
	}
	for _, name := range [...]string{"catch", "catchAll"} {
		if tp.IsNodeReferenceToEffectModuleApi(step.Callee(), name) {
			return name, true
		}
	}
	return "", false
}

func hasSingleArgument(step typeparser.PipingFlowTransformation) bool {
	return step.Callee() != nil && step.Args().Len() == 1 && step.Args().At(0).Kind != ast.KindSpreadElement
}

// matchToResultOrOptionFixable reports whether the replaced steps can be
// dropped: their callees run no code when evaluated, and no explicit type
// argument depends on the old output type. The dropped handlers are always
// side-effect-free references or function literals, as the classifiers
// require.
func matchToResultOrOptionFixable(tp *typeparser.TypeParser, c *checker.Checker, flow *typeparser.PipingFlow, start, end int) bool {
	for i := start; i <= end; i++ {
		step := flow.Transformations().At(i)
		if !isSideEffectFreeReference(tp, c, step.Callee()) || step.Kind() == typeparser.TransformationKindCallWithOptionals {
			return false
		}
	}
	return !mapMapErrorPathHasTypeArguments(flow.Transformations().At(start), flow.Transformations().At(end))
}
