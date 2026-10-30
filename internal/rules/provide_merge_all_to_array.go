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

// ProvideMergeAllToArray suggests passing layers to Effect.provide as an array,
// since Effect.provide already merges an array of layers with Layer.mergeAll.
var ProvideMergeAllToArray = rule.Rule{
	Name:            "provideMergeAllToArray",
	Group:           "style",
	Description:     "Suggests passing an array of layers to Effect.provide instead of wrapping them in Layer.mergeAll",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Effect_provide_accepts_an_array_of_layers_and_merges_them_itself_pass_the_layers_as_an_array_instead_of_wrapping_them_in_Layer_mergeAll_effect_provideMergeAllToArray.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeProvideMergeAllToArray(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diagnostics := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diagnostics[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.Effect_provide_accepts_an_array_of_layers_and_merges_them_itself_pass_the_layers_as_an_array_instead_of_wrapping_them_in_Layer_mergeAll_effect_provideMergeAllToArray,
				nil,
			)
		}
		return diagnostics
	},
}

// ProvideMergeAllToArrayMatch holds the nodes shared by the diagnostic and its quick fix.
type ProvideMergeAllToArrayMatch struct {
	SourceFile *ast.SourceFile
	Location   core.TextRange // Error range of the Layer.mergeAll(...) call
	Argument   *ast.Node      // The provide argument as written, possibly parenthesized
	MergeAll   *ast.Node      // The Layer.mergeAll(...) call inside Argument
}

// AnalyzeProvideMergeAllToArray finds Effect.provide calls whose layer argument
// is written directly as a Layer.mergeAll(...) call.
func AnalyzeProvideMergeAllToArray(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []ProvideMergeAllToArrayMatch {
	if tp == nil || sf == nil {
		return nil
	}

	var matches []ProvideMergeAllToArrayMatch
	for _, flow := range tp.PipingFlows(sf, true).All() {
		for index := range flow.Transformations().All() {
			provide := flow.Transformations().At(index)
			argument := provideLayerArgument(provide)
			if argument == nil {
				continue
			}
			mergeAll := ast.SkipParentheses(argument)
			if !isPlainMergeAllCallShape(mergeAll) ||
				(provide.TypeArguments() != nil && len(provide.TypeArguments().Nodes) > 0) ||
				!tp.IsNodeReferenceToEffectModuleApi(provide.Callee(), "provide") ||
				!tp.IsNodeReferenceToEffectLayerModuleApi(mergeAll.AsCallExpression().Expression, "mergeAll") {
				continue
			}
			// Converting a mergeAll whose layers depend on each other would hide
			// the problem layerMergeAllWithDependencies reports on it.
			if len(analyzeLayerMergeAllCall(tp, c, sf, mergeAll)) > 0 {
				continue
			}

			matches = append(matches, ProvideMergeAllToArrayMatch{
				SourceFile: sf,
				Location:   scanner.GetErrorRangeForNode(sf, mergeAll),
				Argument:   argument,
				MergeAll:   mergeAll,
			})
		}
	}
	return matches
}

// provideLayerArgument returns the layer argument of a provide-like transformation.
// An unapplied single-argument call such as Effect.provide(layer) is a call
// transformation whose subject is the layer, so its argument is read from the call.
func provideLayerArgument(transformation typeparser.PipingFlowTransformation) *ast.Node {
	callee := transformation.Callee()
	if callee == nil {
		return nil
	}
	if transformation.Args().Len() > 0 {
		return transformation.Args().At(0)
	}
	if transformation.Kind() != typeparser.TransformationKindCall {
		return nil
	}
	parent := callee.Parent
	if parent == nil || parent.Kind != ast.KindCallExpression {
		return nil
	}
	call := parent.AsCallExpression()
	if call.Expression != callee || call.Arguments == nil || len(call.Arguments.Nodes) != 1 {
		return nil
	}
	return call.Arguments.Nodes[0]
}

// isPlainMergeAllCallShape checks the syntax a quick fix can rewrite to an array:
// a non-optional call without explicit type arguments and with at least one argument.
func isPlainMergeAllCallShape(node *ast.Node) bool {
	if node == nil || node.Kind != ast.KindCallExpression {
		return false
	}
	call := node.AsCallExpression()
	return call.QuestionDotToken == nil &&
		(call.TypeArguments == nil || len(call.TypeArguments.Nodes) == 0) &&
		call.Arguments != nil && len(call.Arguments.Nodes) > 0
}
