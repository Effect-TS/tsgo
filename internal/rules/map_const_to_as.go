package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/readonly"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

// MapConstToAs suggests Effect.as when an Effect.map callback ignores its
// input and returns a constant.
var MapConstToAs = rule.Rule{
	Name:            "mapConstToAs",
	Group:           "style",
	Description:     "Suggests using Effect.as instead of Effect.map when the callback ignores its input and returns a constant",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Effect_as_expresses_replacing_the_success_value_with_a_constant_directly_effect_mapConstToAs.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeMapConstToAs(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diags[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.Effect_as_expresses_replacing_the_success_value_with_a_constant_directly_effect_mapConstToAs,
				nil,
			)
		}
		return diags
	},
}

// MapConstToAsMatch holds the nodes needed by the diagnostic and quick fix.
type MapConstToAsMatch struct {
	SourceFile       *ast.SourceFile
	Location         core.TextRange
	Transformation   *typeparser.PipingFlowTransformation
	EffectModuleNode *ast.Node // The Effect module identifier (e.g., "Effect" in Effect.map)
	ConstantValue    *ast.Node // The returned constant, with parentheses skipped
}

// AnalyzeMapConstToAs finds data-last and data-first Effect.map calls whose
// callback ignores its input and returns a value that is stable when the
// Effect is constructed, so Effect.as can capture it once.
func AnalyzeMapConstToAs(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile) []MapConstToAsMatch {
	var matches []MapConstToAsMatch
	for _, flow := range tp.PipingFlows(sf, true).All() {
		for index := range flow.Transformations().All() {
			transformation := flow.Transformations().At(index)
			callee := transformation.Callee()
			if callee == nil || transformation.Args().Len() != 1 ||
				transformation.TypeArguments() != nil && len(transformation.TypeArguments().Nodes) > 0 {
				continue
			}

			lazy := typeparser.ParseLazyExpression(transformation.Args().At(0), typeparser.LazyExpressionNone)
			// A return type annotation pins Effect.map's success type independently
			// of the value; Effect.as infers it from the value alone.
			if lazy == nil || lazy.Expression() == nil || lazy.Node().Type() != nil ||
				!isIgnorableMapParameterList(lazy.Params()) {
				continue
			}
			value := ast.SkipParentheses(lazy.Expression())
			// Void-producing bodies are reported by effectMapVoid as Effect.asVoid.
			if isVoidExpression(value) ||
				!tp.IsNodeReferenceToEffectModuleApi(callee, "map") ||
				!tp.IsExpressionValueStableAtLocation(value, callee) {
				continue
			}

			var effectModuleNode *ast.Node
			if callee.Kind == ast.KindPropertyAccessExpression {
				effectModuleNode = callee.AsPropertyAccessExpression().Expression
			}
			matches = append(matches, MapConstToAsMatch{
				SourceFile:       sf,
				Location:         scanner.GetErrorRangeForNode(sf, callee),
				Transformation:   &transformation,
				EffectModuleNode: effectModuleNode,
				ConstantValue:    value,
			})
		}
	}
	return matches
}

// isIgnorableMapParameterList accepts no parameters, or a single plain
// identifier parameter. Stable values never resolve to that parameter, so it
// is unused whenever the returned value is stable.
func isIgnorableMapParameterList(params readonly.Slice[*ast.Node]) bool {
	switch params.Len() {
	case 0:
		return true
	case 1:
		param := params.At(0)
		declaration := param.AsParameterDeclaration()
		return !ast.IsThisParameter(param) && ast.IsIdentifier(param.Name()) &&
			declaration.DotDotDotToken == nil && declaration.Initializer == nil
	}
	return false
}
