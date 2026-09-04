package rules

import (
	"sort"

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
	// CanFix reports whether the match is a standalone Effect.all call the
	// quick fix can rewrite. Data-last Effect.all references inside piping
	// flows are diagnostic-only, since the fix would have to restructure the
	// surrounding pipe.
	CanFix bool
}

// AnalyzeAllOfMapToForEach finds Effect.all over an effectful Array#map whose
// receiver is array-like, in both the direct data-first form and the data-last
// form expressed through piping flows (e.g. pipe(xs.map(f), Effect.all)).
func AnalyzeAllOfMapToForEach(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []AllOfMapToForEachMatch {
	var matches []AllOfMapToForEachMatch

	// Direct data-first calls: Effect.all(xs.map(f), options?)
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

	// Data-last Effect.all references inside piping flows. A bare reference is
	// invisible to the call walk above because it never appears as a call.
	for _, flow := range tp.PipingFlows(sf, true) {
		for i := range flow.Transformations {
			transformation := &flow.Transformations[i]
			if len(transformation.Args) != 0 ||
				(transformation.Kind != typeparser.TransformationKindPipe && transformation.Kind != typeparser.TransformationKindPipeable) ||
				transformation.Callee == nil ||
				!tp.IsNodeReferenceToEffectModuleApi(transformation.Callee, "all") {
				continue
			}
			if i == 0 {
				continue
			}
			mapNode := transformationApplicationNode(&flow.Transformations[i-1])
			if match, ok := analyzeAllOfMapToForEachReceiver(tp, c, sf, transformation.Callee, mapNode, nil, false); ok {
				matches = append(matches, match)
			}
		}
	}

	sort.Slice(matches, func(i, j int) bool {
		return matches[i].Location.Pos() < matches[j].Location.Pos()
	})
	return matches
}

// transformationApplicationNode returns the call expression that applies the
// transformation's callee, when that application is represented in the tree.
// Bare pipeable references have no application of their own and return nil.
func transformationApplicationNode(transformation *typeparser.PipingFlowTransformation) *ast.Node {
	if transformation == nil || transformation.Callee == nil || transformation.Callee.Parent == nil ||
		!ast.IsCallExpression(transformation.Callee.Parent) {
		return nil
	}
	call := transformation.Callee.Parent.AsCallExpression()
	if call == nil || call.Expression != transformation.Callee {
		return nil
	}
	return call.AsNode()
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

	var options *ast.Node
	if len(allCall.Arguments.Nodes) == 2 {
		options = allCall.Arguments.Nodes[1]
	}
	match, ok := analyzeAllOfMapToForEachReceiver(tp, c, sf, allCall.Expression, allCall.Arguments.Nodes[0], options, hasCallTypeArguments(allCall))
	if !ok {
		return AllOfMapToForEachMatch{}, false
	}
	match.CallNode = node
	match.CanFix = true
	return match, true
}

// analyzeAllOfMapToForEachReceiver validates the xs.map(f) shape feeding an
// Effect.all node and assembles the match.
func analyzeAllOfMapToForEachReceiver(
	tp *typeparser.TypeParser,
	c *checker.Checker,
	sf *ast.SourceFile,
	allNode *ast.Node,
	mapNode *ast.Node,
	options *ast.Node,
	allTypeArguments bool,
) (AllOfMapToForEachMatch, bool) {
	mapNode = ast.SkipParentheses(mapNode)
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
	if mapResultType == nil || tp.EffectType(c.GetNumberIndexType(mapResultType)) == nil {
		return AllOfMapToForEachMatch{}, false
	}

	if options != nil {
		optionsType := tp.GetTypeAtLocation(options)
		if optionsType != nil && c.GetPropertyOfType(optionsType, "mode") != nil {
			return AllOfMapToForEachMatch{}, false
		}
	}

	var effectModule *ast.Node
	if allNode.Kind == ast.KindPropertyAccessExpression {
		effectModule = allNode.AsPropertyAccessExpression().Expression
	}

	return AllOfMapToForEachMatch{
		SourceFile:       sf,
		Location:         scanner.GetErrorRangeForNode(sf, allNode),
		EffectModule:     effectModule,
		Receiver:         mapAccess.Expression,
		Callback:         callback,
		Options:          options,
		HasTypeArguments: allTypeArguments || hasCallTypeArguments(mapCall),
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
