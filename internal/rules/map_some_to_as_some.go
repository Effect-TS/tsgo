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

// MapSomeToAsSome suggests Effect.asSome when Effect.map only wraps the
// success value with Option.some.
var MapSomeToAsSome = rule.Rule{
	Name:            "mapSomeToAsSome",
	Group:           "style",
	Description:     "Suggests using Effect.asSome instead of Effect.map when the mapper only wraps the success value with Option.some",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Effect_asSome_expresses_wrapping_the_success_value_in_Option_some_directly_effect_mapSomeToAsSome.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeMapSomeToAsSome(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diags[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.Effect_asSome_expresses_wrapping_the_success_value_in_Option_some_directly_effect_mapSomeToAsSome,
				nil,
			)
		}
		return diags
	},
}

// MapSomeToAsSomeMatch holds the nodes needed by the diagnostic and quick fix.
type MapSomeToAsSomeMatch struct {
	SourceFile       *ast.SourceFile
	Location         core.TextRange
	CallNode         *ast.Node
	EffectModuleNode *ast.Node
	SubjectNode      *ast.Node
}

// AnalyzeMapSomeToAsSome finds data-last and data-first Effect.map calls whose
// mapper is Option.some or the exact eta-expansion value => Option.some(value).
func AnalyzeMapSomeToAsSome(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []MapSomeToAsSomeMatch {
	var matches []MapSomeToAsSomeMatch
	seen := make(map[*ast.Node]struct{})
	for _, flow := range tp.PipingFlows(sf, true) {
		for _, transformation := range flow.Transformations {
			if len(transformation.Args) != 1 || transformation.Node == nil || transformation.Node.Kind != ast.KindCallExpression ||
				transformation.Callee == nil || transformation.Callee.Kind != ast.KindPropertyAccessExpression ||
				!tp.IsNodeReferenceToEffectModuleApi(transformation.Callee, "map") {
				continue
			}

			call := transformation.Node.AsCallExpression()
			if call == nil || call.TypeArguments != nil && len(call.TypeArguments.Nodes) > 0 ||
				!isOptionSomeMapper(tp, c, transformation.Args[0]) {
				continue
			}
			if _, ok := seen[transformation.Node]; ok {
				continue
			}

			var subject *ast.Node
			if transformation.Kind == typeparser.TransformationKindDataFirst || transformation.Kind == typeparser.TransformationKindDataLast {
				parsed := tp.DataFirstOrLastCall(transformation.Node)
				if parsed == nil {
					continue
				}
				subject = parsed.Subject
			}

			propertyAccess := transformation.Callee.AsPropertyAccessExpression()
			seen[transformation.Node] = struct{}{}
			matches = append(matches, MapSomeToAsSomeMatch{
				SourceFile:       sf,
				Location:         scanner.GetErrorRangeForNode(sf, transformation.Callee),
				CallNode:         transformation.Node,
				EffectModuleNode: propertyAccess.Expression,
				SubjectNode:      subject,
			})
		}
	}
	return matches
}

func isOptionSomeMapper(tp *typeparser.TypeParser, c *checker.Checker, node *ast.Node) bool {
	node = ast.SkipParentheses(node)
	if node == nil {
		return false
	}
	if tp.IsNodeReferenceToEffectOptionModuleApi(node, "some") {
		return true
	}
	lazy := typeparser.ParseLazyExpression(node, false)
	if lazy == nil || lazy.Node.Kind != ast.KindArrowFunction || ast.GetCombinedModifierFlags(lazy.Node)&ast.ModifierFlagsAsync != 0 ||
		len(lazy.Params) != 1 {
		return false
	}
	parameter := lazy.Params[0]
	if parameter == nil || parameter.Name() == nil || parameter.Name().Kind != ast.KindIdentifier {
		return false
	}
	parameterDeclaration := parameter.AsParameterDeclaration()
	if parameterDeclaration == nil || parameterDeclaration.DotDotDotToken != nil || parameterDeclaration.QuestionToken != nil ||
		parameterDeclaration.Type != nil || parameterDeclaration.Initializer != nil {
		return false
	}

	body := ast.SkipParentheses(lazy.Expression)
	if body == nil || body.Kind != ast.KindCallExpression {
		return false
	}
	someCall := body.AsCallExpression()
	if someCall == nil || someCall.Expression == nil ||
		someCall.TypeArguments != nil && len(someCall.TypeArguments.Nodes) > 0 ||
		someCall.Arguments == nil || len(someCall.Arguments.Nodes) != 1 ||
		!tp.IsNodeReferenceToEffectOptionModuleApi(someCall.Expression, "some") {
		return false
	}

	argument := ast.SkipParentheses(someCall.Arguments.Nodes[0])
	if argument == nil || argument.Kind != ast.KindIdentifier {
		return false
	}
	parameterSymbol := tp.GetSymbolAtLocation(parameter.Name())
	argumentSymbol := tp.GetSymbolAtLocation(argument)
	return parameterSymbol != nil && argumentSymbol != nil &&
		checker.Checker_getSymbolIfSameReference(c, parameterSymbol, argumentSymbol) != nil
}
