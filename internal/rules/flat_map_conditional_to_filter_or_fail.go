// Package rules contains all Effect diagnostic rule implementations.
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

// FlatMapConditionalToFilterOrFail suggests Effect.filterOrFail or
// Effect.filterOrElse when a flatMap callback implements an identity filter.
var FlatMapConditionalToFilterOrFail = rule.Rule{
	Name:            "flatMapConditionalToFilterOrFail",
	Group:           "style",
	Description:     "Suggests Effect.filterOrFail or Effect.filterOrElse when Effect.flatMap conditionally passes its input through with Effect.succeed",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Effect_0_expresses_this_conditional_validation_more_directly_than_Effect_flatMap_with_an_identity_Effect_succeed_branch_effect_flatMapConditionalToFilterOrFail.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeFlatMapConditionalToFilterOrFail(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diagnostics := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diagnostics[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.Effect_0_expresses_this_conditional_validation_more_directly_than_Effect_flatMap_with_an_identity_Effect_succeed_branch_effect_flatMapConditionalToFilterOrFail,
				nil,
				match.PreferredMethodName,
			)
		}
		return diagnostics
	},
}

// FlatMapConditionalToFilterOrFailMatch holds the source evidence needed by
// the diagnostic and its quick fix.
type FlatMapConditionalToFilterOrFailMatch struct {
	SourceFile          *ast.SourceFile
	Location            core.TextRange
	Transformation      *typeparser.PipingFlowTransformation
	EffectModuleNode    *ast.Node
	ParameterNode       *ast.Node
	PredicateNode       *ast.Node
	FallbackNode        *ast.Node
	PreferredMethodName string
	NegatePredicate     bool
	CanFix              bool
}

// AnalyzeFlatMapConditionalToFilterOrFail finds flatMap transformations whose
// callback is a two-way conditional with one identity Effect.succeed branch.
func AnalyzeFlatMapConditionalToFilterOrFail(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []FlatMapConditionalToFilterOrFailMatch {
	if tp == nil || c == nil || sf == nil {
		return nil
	}

	var matches []FlatMapConditionalToFilterOrFailMatch
	for _, flow := range tp.PipingFlows(sf, true) {
		for index := range flow.Transformations {
			transformation := &flow.Transformations[index]
			callee := transformation.Callee
			args := transformation.Args
			if callee == nil || len(args) != 1 || !tp.IsNodeReferenceToEffectModuleApi(callee, "flatMap") {
				continue
			}
			parsed := typeparser.ParseReturningDispatch(args[0])
			if parsed == nil || parsed.Dispatch == nil || len(parsed.Params) != 1 ||
				len(parsed.Dispatch.Branches) != 1 || parsed.Dispatch.Fallback == nil ||
				parsed.Dispatch.Branches[0].Condition.Kind != typeparser.DispatchConditionPredicate ||
				!isSynchronousFunction(parsed.Node) {
				continue
			}

			parameter := parsed.Params[0]
			if parameter == nil || parameter.Name() == nil || parameter.Name().Kind != ast.KindIdentifier {
				continue
			}
			declaration := parameter.AsParameterDeclaration()
			if declaration == nil || declaration.DotDotDotToken != nil || declaration.Initializer != nil {
				continue
			}
			parameterSymbol := tp.GetSymbolAtLocation(parameter.Name())
			if parameterSymbol == nil || checker.Checker_isSymbolAssigned(c, parameterSymbol) {
				continue
			}

			branch := parsed.Dispatch.Branches[0]
			branchIdentity := isIdentitySucceed(tp, c, branch.Result, parameterSymbol)
			fallbackIdentity := isIdentitySucceed(tp, c, parsed.Dispatch.Fallback, parameterSymbol)
			if branchIdentity == fallbackIdentity {
				continue
			}

			fallback := branch.Result
			negatePredicate := true
			if branchIdentity {
				fallback = parsed.Dispatch.Fallback
				negatePredicate = false
			}

			preferredMethod := "filterOrElse"
			fallbackNode := fallback
			if failure := effectFailArgument(tp, fallback); failure != nil {
				preferredMethod = "filterOrFail"
				fallbackNode = failure
			} else if !isEffectExpression(tp, fallback) {
				continue
			}

			effectModule := effectModuleExpression(callee)
			match := FlatMapConditionalToFilterOrFailMatch{
				SourceFile:          sf,
				Location:            scanner.GetErrorRangeForNode(sf, callee),
				Transformation:      transformation,
				EffectModuleNode:    effectModule,
				ParameterNode:       parameter,
				PredicateNode:       branch.Condition.Subject,
				FallbackNode:        fallbackNode,
				PreferredMethodName: preferredMethod,
				NegatePredicate:     negatePredicate,
				CanFix:              effectModule != nil,
			}

			matches = append(matches, match)
		}
	}
	return matches
}

func isIdentitySucceed(tp *typeparser.TypeParser, c *checker.Checker, expression *ast.Node, parameterSymbol *ast.Symbol) bool {
	expression = ast.SkipParentheses(expression)
	if expression == nil || expression.Kind != ast.KindCallExpression {
		return false
	}
	call := expression.AsCallExpression()
	if call == nil || call.Expression == nil || call.Arguments == nil || len(call.Arguments.Nodes) != 1 ||
		call.TypeArguments != nil && len(call.TypeArguments.Nodes) > 0 ||
		!tp.IsNodeReferenceToEffectModuleApi(call.Expression, "succeed") {
		return false
	}
	argument := ast.SkipParentheses(call.Arguments.Nodes[0])
	if argument == nil || argument.Kind != ast.KindIdentifier {
		return false
	}
	actualSymbol := tp.GetSymbolAtLocation(argument)
	return actualSymbol != nil && checker.Checker_getSymbolIfSameReference(c, actualSymbol, parameterSymbol) != nil
}

func effectFailArgument(tp *typeparser.TypeParser, expression *ast.Node) *ast.Node {
	expression = ast.SkipParentheses(expression)
	if expression == nil || expression.Kind != ast.KindCallExpression {
		return nil
	}
	call := expression.AsCallExpression()
	if call == nil || call.Expression == nil || call.Arguments == nil || len(call.Arguments.Nodes) != 1 ||
		call.TypeArguments != nil && len(call.TypeArguments.Nodes) > 0 ||
		!tp.IsNodeReferenceToEffectModuleApi(call.Expression, "fail") {
		return nil
	}
	return call.Arguments.Nodes[0]
}
