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

// AllOfMapToForEach suggests using Effect.forEach instead of constructing an
// intermediate array of effects with Array#map and passing it to Effect.all.
var AllOfMapToForEach = rule.Rule{
	Name:            "allOfMapToForEach",
	Group:           "style",
	Description:     "Suggests using Effect.forEach instead of Effect.all over an effectful Array#map",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Effect_forEach_expresses_this_effectful_array_mapping_more_directly_than_Effect_all_over_Array_map_effect_allOfMapToForEach.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeAllOfMapToForEach(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diagnostics := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diagnostics[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.Effect_forEach_expresses_this_effectful_array_mapping_more_directly_than_Effect_all_over_Array_map_effect_allOfMapToForEach,
				nil,
			)
		}
		return diagnostics
	},
}

// AllOfMapToForEachMatch holds the nodes needed by the diagnostic and its fix.
type AllOfMapToForEachMatch struct {
	SourceFile       *ast.SourceFile
	Location         core.TextRange
	CallNode         *ast.Node
	EffectModule     *ast.Node
	Receiver         *ast.Node
	Callback         *ast.Node
	Options          *ast.Node
	HasTypeArguments bool
}

// AnalyzeAllOfMapToForEach finds Effect.all(xs.map(f), options?) calls whose
// receiver is array-like and whose map callback produces Effects.
func AnalyzeAllOfMapToForEach(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []AllOfMapToForEachMatch {
	var matches []AllOfMapToForEachMatch
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}

		if match, ok := analyzeAllOfMapToForEachCall(tp, c, sf, node); ok {
			matches = append(matches, match)
		}

		node.ForEachChild(walk)
		return false
	}

	walk(sf.AsNode())
	return matches
}

func analyzeAllOfMapToForEachCall(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile, node *ast.Node) (AllOfMapToForEachMatch, bool) {
	if node.Kind != ast.KindCallExpression {
		return AllOfMapToForEachMatch{}, false
	}
	allCall := node.AsCallExpression()
	if allCall == nil || allCall.Expression == nil || allCall.Arguments == nil || len(allCall.Arguments.Nodes) < 1 || len(allCall.Arguments.Nodes) > 2 {
		return AllOfMapToForEachMatch{}, false
	}
	if !tp.IsNodeReferenceToEffectModuleApi(allCall.Expression, "all") || containsSpreadElement(allCall.Arguments.Nodes) {
		return AllOfMapToForEachMatch{}, false
	}

	mapNode := ast.SkipParentheses(allCall.Arguments.Nodes[0])
	if mapNode == nil || mapNode.Kind != ast.KindCallExpression {
		return AllOfMapToForEachMatch{}, false
	}
	mapCall := mapNode.AsCallExpression()
	if mapCall == nil || mapCall.Expression == nil || mapCall.Expression.Kind != ast.KindPropertyAccessExpression || mapCall.Arguments == nil || len(mapCall.Arguments.Nodes) != 1 || containsSpreadElement(mapCall.Arguments.Nodes) {
		return AllOfMapToForEachMatch{}, false
	}
	mapAccess := mapCall.Expression.AsPropertyAccessExpression()
	if mapAccess == nil || mapAccess.Name() == nil || mapAccess.Name().Text() != "map" || mapAccess.Expression == nil {
		return AllOfMapToForEachMatch{}, false
	}

	receiverType := tp.GetTypeAtLocation(mapAccess.Expression)
	isArrayReceiver := receiverType != nil && (checker.Checker_isArrayType(c, receiverType) || checker.Checker_isReadonlyArrayType(c, receiverType))
	if !isArrayReceiver {
		return AllOfMapToForEachMatch{}, false
	}

	callback := mapCall.Arguments.Nodes[0]
	mapResultType := tp.GetTypeAtLocation(mapNode)
	if mapResultType == nil || tp.EffectType(c.GetNumberIndexType(mapResultType), callback) == nil {
		return AllOfMapToForEachMatch{}, false
	}

	var options *ast.Node
	if len(allCall.Arguments.Nodes) == 2 {
		options = allCall.Arguments.Nodes[1]
		optionsType := tp.GetTypeAtLocation(options)
		if optionsType != nil && c.GetPropertyOfType(optionsType, "mode") != nil {
			return AllOfMapToForEachMatch{}, false
		}
	}

	var effectModule *ast.Node
	if allCall.Expression.Kind == ast.KindPropertyAccessExpression {
		effectModule = allCall.Expression.AsPropertyAccessExpression().Expression
	}

	return AllOfMapToForEachMatch{
		SourceFile:       sf,
		Location:         scanner.GetErrorRangeForNode(sf, allCall.Expression),
		CallNode:         node,
		EffectModule:     effectModule,
		Receiver:         mapAccess.Expression,
		Callback:         callback,
		Options:          options,
		HasTypeArguments: hasCallTypeArguments(allCall) || hasCallTypeArguments(mapCall),
	}, true
}
func containsSpreadElement(nodes []*ast.Node) bool {
	for _, node := range nodes {
		if node == nil || node.Kind == ast.KindSpreadElement {
			return true
		}
	}
	return false
}

func hasCallTypeArguments(call *ast.CallExpression) bool {
	return call != nil && call.TypeArguments != nil && len(call.TypeArguments.Nodes) > 0
}
