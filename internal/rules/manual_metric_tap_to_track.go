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

// ManualMetricTapToTrack suggests the Effect.track* combinators when a tap
// variant exists only to feed one metric through Metric.update.
var ManualMetricTapToTrack = rule.Rule{
	Name:            "manualMetricTapToTrack",
	Group:           "style",
	Description:     "Suggests Effect.trackSuccesses/trackErrors/trackDefects when an Effect.tap/tapError/tapDefect only updates a metric",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v4"},
	Codes: []int32{
		tsdiag.Effect_0_expresses_recording_a_metric_on_every_1_directly_effect_manualMetricTapToTrack.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeManualMetricTapToTrack(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diags[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.Effect_0_expresses_recording_a_metric_on_every_1_directly_effect_manualMetricTapToTrack,
				nil,
				match.TrackAPI,
				match.Channel,
			)
		}
		return diags
	},
}

// ManualMetricTapToTrackKind selects the shape of the track* replacement.
type ManualMetricTapToTrackKind uint8

const (
	// ManualMetricTapToTrackVerbatim feeds the callback parameter as is: track*(metric).
	ManualMetricTapToTrackVerbatim ManualMetricTapToTrackKind = iota
	// ManualMetricTapToTrackMapper computes the input per run: track*(metric, (params) => input).
	ManualMetricTapToTrackMapper
	// ManualMetricTapToTrackConstantInput comes from tap's direct-effect overload,
	// whose input is evaluated once: trackSuccesses(Metric.withConstantInput(metric, input)).
	ManualMetricTapToTrackConstantInput
)

// ManualMetricTapToTrackMatch holds the nodes needed by the diagnostic and quick fix.
type ManualMetricTapToTrackMatch struct {
	SourceFile       *ast.SourceFile
	Location         core.TextRange
	Transformation   *typeparser.PipingFlowTransformation
	EffectModuleNode *ast.Node // The Effect module identifier (e.g., "Effect" in Effect.tap)
	MetricModuleNode *ast.Node // The Metric module identifier (e.g., "Metric" in Metric.update)
	TrackAPI         string    // trackSuccesses, trackErrors or trackDefects
	Channel          string    // success, error or defect
	Kind             ManualMetricTapToTrackKind
	Metric           *ast.Node // The updated metric, with parentheses skipped
	Input            *ast.Node // The metric input as written
	Params           readonly.Slice[*ast.Node]
}

type manualMetricTapAPI struct {
	tap     string
	track   string
	channel string
}

var manualMetricTapAPIs = [...]manualMetricTapAPI{
	{tap: "tap", track: "trackSuccesses", channel: "success"},
	{tap: "tapError", track: "trackErrors", channel: "error"},
	{tap: "tapDefect", track: "trackDefects", channel: "defect"},
}

// AnalyzeManualMetricTapToTrack finds Effect.tap/tapError/tapDefect calls whose
// argument only performs a data-first Metric.update, either as the whole body
// of a callback or as tap's direct-effect argument.
func AnalyzeManualMetricTapToTrack(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile) []ManualMetricTapToTrackMatch {
	if tp.SupportedEffectVersion() != typeparser.EffectMajorV4 {
		return nil
	}
	var matches []ManualMetricTapToTrackMatch
	for _, flow := range tp.PipingFlows(sf, true).All() {
		for index := range flow.Transformations().All() {
			transformation := flow.Transformations().At(index)
			callee := transformation.Callee()
			if callee == nil || transformation.Args().Len() != 1 ||
				transformation.TypeArguments() != nil && len(transformation.TypeArguments().Nodes) > 0 {
				continue
			}

			arg := transformation.Args().At(0)
			var kind ManualMetricTapToTrackKind
			var update *ast.CallExpression
			var params readonly.Slice[*ast.Node]
			if lazy := typeparser.ParseLazyExpression(arg, typeparser.LazyExpressionNone); lazy != nil {
				if lazy.Expression() == nil || hasThisParameter(lazy.Params()) {
					continue
				}
				update = metricUpdateCall(tp, lazy.Expression())
				// tap callbacks evaluate the metric on every run while track*
				// evaluates it once, so it must be the same value at the call site.
				if update == nil || !tp.IsExpressionValueStableAtLocation(update.Arguments.Nodes[0], callee) {
					continue
				}
				params = lazy.Params()
				kind = ManualMetricTapToTrackMapper
				if isParameterVerbatim(tp, params, update.Arguments.Nodes[1]) {
					kind = ManualMetricTapToTrackVerbatim
				} else if lazy.Node().Kind == ast.KindFunctionExpression &&
					usesFunctionExpressionBindings(tp, lazy.Node(), params, update.Arguments.Nodes[1]) {
					// The mapper is an arrow function that keeps the parameters and the input.
					continue
				}
			} else {
				// The direct-effect overload builds the update once, as track* does.
				update = metricUpdateCall(tp, arg)
				if update == nil {
					continue
				}
				kind = ManualMetricTapToTrackConstantInput
			}

			api := resolveManualMetricTapAPI(tp, callee)
			if api == nil || kind == ManualMetricTapToTrackConstantInput && api.tap != "tap" {
				continue
			}

			var effectModuleNode *ast.Node
			if callee.Kind == ast.KindPropertyAccessExpression {
				effectModuleNode = callee.AsPropertyAccessExpression().Expression
			}
			var metricModuleNode *ast.Node
			if update.Expression.Kind == ast.KindPropertyAccessExpression {
				metricModuleNode = update.Expression.AsPropertyAccessExpression().Expression
			}
			matches = append(matches, ManualMetricTapToTrackMatch{
				SourceFile:       sf,
				Location:         scanner.GetErrorRangeForNode(sf, callee),
				Transformation:   &transformation,
				EffectModuleNode: effectModuleNode,
				MetricModuleNode: metricModuleNode,
				TrackAPI:         api.track,
				Channel:          api.channel,
				Kind:             kind,
				Metric:           ast.SkipParentheses(update.Arguments.Nodes[0]),
				Input:            update.Arguments.Nodes[1],
				Params:           params,
			})
		}
	}
	return matches
}

// metricUpdateCall returns node as a plain data-first Metric.update(metric, input) call.
func metricUpdateCall(tp *typeparser.TypeParser, node *ast.Node) *ast.CallExpression {
	node = ast.SkipParentheses(node)
	if node == nil || node.Kind != ast.KindCallExpression {
		return nil
	}
	call := node.AsCallExpression()
	if call.Expression == nil || call.QuestionDotToken != nil ||
		call.TypeArguments != nil && len(call.TypeArguments.Nodes) > 0 ||
		call.Arguments == nil || len(call.Arguments.Nodes) != 2 {
		return nil
	}
	for _, argument := range call.Arguments.Nodes {
		if argument.Kind == ast.KindSpreadElement {
			return nil
		}
	}
	if !tp.IsNodeReferenceToEffectMetricModuleApi(call.Expression, "update") {
		return nil
	}
	return call
}

func hasThisParameter(params readonly.Slice[*ast.Node]) bool {
	for _, param := range params.All() {
		if ast.IsThisParameter(param) {
			return true
		}
	}
	return false
}

// isParameterVerbatim reports whether input is the single plain callback parameter itself.
func isParameterVerbatim(tp *typeparser.TypeParser, params readonly.Slice[*ast.Node], input *ast.Node) bool {
	if params.Len() != 1 {
		return false
	}
	param := params.At(0)
	declaration := param.AsParameterDeclaration()
	if !ast.IsIdentifier(param.Name()) || declaration.DotDotDotToken != nil || declaration.Initializer != nil {
		return false
	}
	input = ast.SkipParentheses(input)
	if input == nil || !ast.IsIdentifier(input) {
		return false
	}
	symbol := tp.GetSymbolAtLocation(input)
	return symbol != nil && symbol == tp.GetSymbolAtLocation(param.Name())
}

// usesFunctionExpressionBindings reports whether the parameters or the input of
// the function expression fn read a binding that fn itself provides: this,
// super, arguments, new.target or fn's own name. An arrow function in the
// same position resolves all of them differently.
func usesFunctionExpressionBindings(tp *typeparser.TypeParser, fn *ast.Node, params readonly.Slice[*ast.Node], input *ast.Node) bool {
	var self *ast.Symbol
	if name := fn.Name(); name != nil {
		self = tp.GetSymbolAtLocation(name)
	}
	// own is false inside nested non-arrow functions, which bind their own
	// this, super, arguments and new.target; fn's name is visible everywhere.
	var visit func(node *ast.Node, own bool) bool
	visit = func(node *ast.Node, own bool) bool {
		if node == nil {
			return false
		}
		switch node.Kind {
		case ast.KindThisKeyword, ast.KindSuperKeyword, ast.KindMetaProperty:
			if own {
				return true
			}
		case ast.KindIdentifier:
			return own && node.Text() == "arguments" || self != nil && tp.GetSymbolAtLocation(node) == self
		case ast.KindClassExpression:
			// Computed names, heritage clauses and static blocks mix both scopes.
			if own {
				return true
			}
		case ast.KindFunctionExpression, ast.KindFunctionDeclaration, ast.KindMethodDeclaration,
			ast.KindGetAccessor, ast.KindSetAccessor, ast.KindConstructor:
			name := node.Name()
			return node.ForEachChild(func(child *ast.Node) bool {
				// A computed method name is evaluated in the enclosing scope.
				return visit(child, own && child == name)
			})
		}
		return node.ForEachChild(func(child *ast.Node) bool { return visit(child, own) })
	}
	for _, param := range params.All() {
		if visit(param, true) {
			return true
		}
	}
	return visit(input, true)
}

func resolveManualMetricTapAPI(tp *typeparser.TypeParser, callee *ast.Node) *manualMetricTapAPI {
	for i := range manualMetricTapAPIs {
		if tp.IsNodeReferenceToEffectModuleApi(callee, manualMetricTapAPIs[i].tap) {
			return &manualMetricTapAPIs[i]
		}
	}
	return nil
}
