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
	Description:     "Suggests Effect.catchTag instead of Effect.catchIf with a _tag equality or Predicate.isTagged predicate",
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
	for _, flow := range tp.PipingFlows(sf, true).All() {
		for index := range flow.Transformations().All() {
			transformation := flow.Transformations().At(index)
			if transformation.Callee() == nil || transformation.Args().Len() != 2 || !tp.IsNodeReferenceToEffectModuleApi(transformation.Callee(), "catchIf") {
				continue
			}
			tag, ok := catchIfTagPredicate(tp, c, transformation.Args().At(0))
			if !ok {
				continue
			}
			matches = append(matches, CatchIfTagToCatchTagMatch{
				SourceFile: sf, Location: scanner.GetErrorRangeForNode(sf, transformation.Callee()),
				Transformation: &transformation,
				Tag:            tag, Handler: transformation.Args().At(1),
				CanFix: true,
			})
		}
	}
	return matches
}

func catchIfTagPredicate(tp *typeparser.TypeParser, c *checker.Checker, predicateNode *ast.Node) (string, bool) {
	lazy := typeparser.ParseLazyExpression(predicateNode, typeparser.LazyExpressionNone)
	if lazy == nil || lazy.Params().Len() != 1 {
		return "", false
	}
	parameter := lazy.Params().At(0)
	if parameter == nil || parameter.Name() == nil || parameter.Name().Kind != ast.KindIdentifier {
		return "", false
	}
	parameterSymbol := tp.GetSymbolAtLocation(parameter.Name())
	if parameterSymbol == nil || checker.Checker_isSymbolAssigned(c, parameterSymbol) {
		return "", false
	}

	tagSubject, tagValue := tp.ParseTagMatch(lazy.Expression())
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
