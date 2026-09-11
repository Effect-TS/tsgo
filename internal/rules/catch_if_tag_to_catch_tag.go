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

var CatchIfTagToCatchTag = rule.Rule{
	Name:            "catchIfTagToCatchTag",
	Group:           "style",
	Description:     "Suggests Effect.catchTag instead of Effect.catchIf with a direct _tag equality predicate",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Effect_catchTag_expresses_tagged_error_recovery_more_directly_than_Effect_catchIf_with_a_tag_equality_predicate_effect_catchIfTagToCatchTag.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeCatchIfTagToCatchTag(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diagnostics := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diagnostics[i] = ctx.NewDiagnostic(match.SourceFile, match.Location, tsdiag.Effect_catchTag_expresses_tagged_error_recovery_more_directly_than_Effect_catchIf_with_a_tag_equality_predicate_effect_catchIfTagToCatchTag, nil)
		}
		return diagnostics
	},
}

type CatchIfTagToCatchTagMatch struct {
	SourceFile     *ast.SourceFile
	Location       core.TextRange
	Transformation *typeparser.PipingFlowTransformation
	Tag            string
	Handler        *ast.Node
	CanFix         bool
}

func AnalyzeCatchIfTagToCatchTag(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []CatchIfTagToCatchTagMatch {
	if tp == nil || c == nil || sf == nil {
		return nil
	}

	var matches []CatchIfTagToCatchTagMatch
	for _, flow := range tp.PipingFlows(sf, true) {
		for index := range flow.Transformations {
			transformation := &flow.Transformations[index]
			if transformation.Callee == nil || !tp.IsNodeReferenceToEffectModuleApi(transformation.Callee, "catchIf") || len(transformation.Args) != 2 {
				continue
			}
			tag, ok := catchIfTagPredicate(tp, c, transformation.Args[0])
			if !ok {
				continue
			}
			matches = append(matches, CatchIfTagToCatchTagMatch{
				SourceFile: sf, Location: scanner.GetErrorRangeForNode(sf, transformation.Callee),
				Transformation: transformation,
				Tag:            tag, Handler: transformation.Args[1],
				CanFix: true,
			})
		}
	}
	return matches
}

func catchIfTagPredicate(tp *typeparser.TypeParser, c *checker.Checker, predicateNode *ast.Node) (string, bool) {
	lazy := typeparser.ParseLazyExpression(predicateNode, typeparser.LazyExpressionNone)
	if lazy == nil || len(lazy.Params) != 1 {
		return "", false
	}
	parameter := lazy.Params[0]
	if parameter == nil || parameter.Name() == nil || parameter.Name().Kind != ast.KindIdentifier {
		return "", false
	}
	parameterSymbol := tp.GetSymbolAtLocation(parameter.Name())
	if parameterSymbol == nil || checker.Checker_isSymbolAssigned(c, parameterSymbol) {
		return "", false
	}

	tagSubject, tagValue := typeparser.ParseTagMatch(lazy.Expression)
	if tagSubject == nil || tagValue == nil || !ast.IsStringLiteral(tagValue) || !isBareParameterTagReference(tp, c, tagSubject, parameterSymbol) {
		return "", false
	}
	tag := tagValue.AsStringLiteral().Text
	tags, ok := literalTaggedUnionTags(tp, c, parameter.Name())
	if !ok {
		return "", false
	}
	_, ok = tags[tag]
	return tag, ok
}
