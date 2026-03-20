package rules

import (
	"strings"

	"github.com/effect-ts/effect-typescript-go/etscore"
	"github.com/effect-ts/effect-typescript-go/internal/rule"
	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// EffectFnOpportunity detects functions that can be rewritten as Effect.fn calls.
var EffectFnOpportunity = rule.Rule{
	Name:            "effectFnOpportunity",
	Group:           "style",
	Description:     "Suggests using Effect.fn for functions that return an Effect",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.Can_be_rewritten_as_a_reusable_function_Colon_0_effect_effectFnOpportunity.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		effectConfig := ctx.Checker.Program().Options().Effect
		matches := AnalyzeEffectFnOpportunity(ctx.Checker, ctx.SourceFile)
		var diags []*ast.Diagnostic
		for i := range matches {
			m := &matches[i]
			fixName := firstAvailableFixName(m.Result, effectConfig)
			if fixName == "" {
				continue
			}
			expectedSignature := buildExpectedSignature(ctx.SourceFile, m, fixName)
			diags = append(diags, ctx.NewDiagnostic(m.SourceFile, m.Location, tsdiag.Can_be_rewritten_as_a_reusable_function_Colon_0_effect_effectFnOpportunity, nil, expectedSignature))
		}
		return diags
	},
}

// EffectFnOpportunityMatch holds the parsed result needed by both the diagnostic rule
// and the quick-fix for the effectFnOpportunity pattern.
type EffectFnOpportunityMatch struct {
	SourceFile *ast.SourceFile
	Location core.TextRange
	Result   *typeparser.EffectFnOpportunityResult
}

// AnalyzeEffectFnOpportunity finds all functions that can be converted to Effect.fn
// in the given source file.
func AnalyzeEffectFnOpportunity(c *checker.Checker, sf *ast.SourceFile) []EffectFnOpportunityMatch {
	var matches []EffectFnOpportunityMatch

	var walk ast.Visitor
	walk = func(n *ast.Node) bool {
		if n == nil {
			return false
		}

		if result := typeparser.ParseEffectFnOpportunity(c, n); result != nil {
			// Report on the name identifier if available, otherwise the function node
			reportNode := result.NameIdentifier
			if reportNode == nil {
				reportNode = result.TargetNode
			}
			matches = append(matches, EffectFnOpportunityMatch{
				SourceFile: sf,
				Location: scanner.GetErrorRangeForNode(sf, reportNode),
				Result:   result,
			})
		}

		n.ForEachChild(walk)
		return false
	}

	walk(sf.AsNode())

	return matches
}

// firstAvailableFixName determines which fix variant would be first (highest priority)
// for the given match, matching the reference implementation's fix ordering.
// Returns empty string when no fix variant passes the config filter.
func firstAvailableFixName(result *typeparser.EffectFnOpportunityResult, effectConfig *etscore.EffectPluginOptions) string {
	// Priority order matches upstream: withSpan > untraced > noSpan > spanInferred > spanSuggested
	if effectConfig.EffectFnIncludes(etscore.EffectFnSpan) && result.ExplicitTraceExpression != nil {
		return "effectFnOpportunity_toEffectFnWithSpan"
	}
	if effectConfig.EffectFnIncludes(etscore.EffectFnUntraced) && result.GeneratorFunction != nil {
		return "effectFnOpportunity_toEffectFnUntraced"
	}
	if effectConfig.EffectFnIncludes(etscore.EffectFnNoSpan) {
		return "effectFnOpportunity_toEffectFnNoSpan"
	}
	if result.ExplicitTraceExpression == nil {
		if effectConfig.EffectFnIncludes(etscore.EffectFnInferredSpan) && result.InferredTraceName != "" {
			return "effectFnOpportunity_toEffectFnSpanInferred"
		}
		if effectConfig.EffectFnIncludes(etscore.EffectFnSuggestedSpan) && result.SuggestedTraceName != "" &&
			(!effectConfig.EffectFnIncludes(etscore.EffectFnInferredSpan) || result.SuggestedTraceName != result.InferredTraceName) {
			return "effectFnOpportunity_toEffectFnSpanSuggested"
		}
	}
	return ""
}

// buildExpectedSignature constructs the human-readable expected signature string
// for the diagnostic message. The format matches the reference implementation:
// it shows how the function would look after conversion with the highest-priority fix.
func buildExpectedSignature(sf *ast.SourceFile, m *EffectFnOpportunityMatch, fixName string) string {
	result := m.Result

	// Get the Effect module name
	effectModuleName := "Effect"
	if result.EffectModule != nil && result.EffectModule.Kind == ast.KindIdentifier {
		effectModuleName = scanner.GetTextOfNode(result.EffectModule)
	}

	// Build type parameter string: <T, U, ...>
	typeParamStr := getTypeParamString(result.TargetNode)

	// Build parameter names string: (x, y, ...)
	paramStr := getParamNamesString(result.TargetNode)

	var fnSignature string
	if result.HasGenBody {
		fnSignature = "function*" + typeParamStr + "(" + paramStr + ") { ... }"
	} else if result.TargetNode != nil && result.TargetNode.Kind == ast.KindArrowFunction {
		fnSignature = typeParamStr + "(" + paramStr + ") => { ... }"
	} else {
		fnSignature = "function" + typeParamStr + "(" + paramStr + ") { ... }"
	}

	pipeArgs := result.PipeArguments
	pipeArgsForWithSpan := pipeArgs
	if len(pipeArgs) > 0 {
		pipeArgsForWithSpan = pipeArgs[:len(pipeArgs)-1]
	}

	pipeArgsSuffix := func(args []*ast.Node) string {
		if len(args) > 0 {
			return ", ...pipeTransformations"
		}
		return ""
	}

	switch fixName {
	case "effectFnOpportunity_toEffectFnWithSpan":
		traceName := ""
		if result.ExplicitTraceExpression != nil {
			traceName = strings.TrimSpace(sf.Text()[result.ExplicitTraceExpression.Pos():result.ExplicitTraceExpression.End()])
		}
		return effectModuleName + ".fn(" + traceName + ")(" + fnSignature + pipeArgsSuffix(pipeArgsForWithSpan) + ")"

	case "effectFnOpportunity_toEffectFnUntraced":
		return effectModuleName + ".fnUntraced(" + fnSignature + pipeArgsSuffix(pipeArgs) + ")"

	case "effectFnOpportunity_toEffectFnNoSpan":
		return effectModuleName + ".fn(" + fnSignature + pipeArgsSuffix(pipeArgs) + ")"

	case "effectFnOpportunity_toEffectFnSpanInferred":
		return effectModuleName + ".fn(\"" + result.InferredTraceName + "\")(" + fnSignature + pipeArgsSuffix(pipeArgs) + ")"

	case "effectFnOpportunity_toEffectFnSpanSuggested":
		return effectModuleName + ".fn(\"" + result.SuggestedTraceName + "\")(" + fnSignature + pipeArgsSuffix(pipeArgs) + ")"

	default:
		return effectModuleName + ".fn(" + fnSignature + ")"
	}
}

// getTypeParamString extracts type parameter names from a function node and returns
// a string like "<T, U>" or "" if there are no type parameters.
func getTypeParamString(fnNode *ast.Node) string {
	typeParams := typeparser.GetFunctionLikeTypeParameters(fnNode)

	if typeParams == nil || len(typeParams.Nodes) == 0 {
		return ""
	}

	var names []string
	for _, tp := range typeParams.Nodes {
		name := tp.AsTypeParameter().Name()
		if name != nil {
			names = append(names, scanner.GetTextOfNode(name))
		}
	}
	if len(names) == 0 {
		return ""
	}
	return "<" + strings.Join(names, ", ") + ">"
}

// getParamNamesString extracts parameter names from a function node and returns
// a string like "x, y" or "" if there are no parameters.
// For destructuring patterns, uses "_" as a placeholder.
func getParamNamesString(fnNode *ast.Node) string {
	params := typeparser.GetFunctionLikeParameters(fnNode)

	if params == nil || len(params.Nodes) == 0 {
		return ""
	}

	var names []string
	for _, p := range params.Nodes {
		pd := p.AsParameterDeclaration()
		name := pd.Name()
		if name != nil && name.Kind == ast.KindIdentifier {
			names = append(names, scanner.GetTextOfNode(name))
		} else {
			names = append(names, "_")
		}
	}
	return strings.Join(names, ", ")
}
