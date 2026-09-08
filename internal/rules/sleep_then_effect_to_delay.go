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

// SleepThenEffectToDelay suggests using Effect.delay instead of sequencing Effect.sleep before an effect.
var SleepThenEffectToDelay = rule.Rule{
	Name:            "sleepThenEffectToDelay",
	Group:           "style",
	Description:     "Sequencing Effect.sleep before an effect re-implements Effect.delay",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Sequencing_Effect_sleep_before_an_effect_re_implements_Effect_delay_effect_sleepThenEffectToDelay.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeSleepThenEffectToDelay(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diags[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.Sequencing_Effect_sleep_before_an_effect_re_implements_Effect_delay_effect_sleepThenEffectToDelay,
				nil,
			)
		}
		return diags
	},
}

// SleepThenEffectToDelayMatch holds match details for diagnostic emission.
type SleepThenEffectToDelayMatch struct {
	SourceFile *ast.SourceFile
	Location   core.TextRange
	Callee     *ast.Node
}

// AnalyzeSleepThenEffectToDelay finds sequencing calls where Effect.sleep is the first operand.
func AnalyzeSleepThenEffectToDelay(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []SleepThenEffectToDelayMatch {
	if tp == nil || c == nil || sf == nil {
		return nil
	}

	var matches []SleepThenEffectToDelayMatch
	visitedCallees := make(map[core.TextRange]bool)

	flows := tp.PipingFlows(sf, true)
	for _, flow := range flows {
		if flow == nil {
			continue
		}

		var sequencer *typeparser.PipingFlowTransformation

		if isDirectEffectSleepCall(tp, flow.Subject.Node) {
			if len(flow.Transformations) > 0 {
				sequencer = &flow.Transformations[0]
			}
		} else if len(flow.Transformations) > 1 && isEffectSleepTransformation(tp, &flow.Transformations[0]) {
			sequencer = &flow.Transformations[1]
		}

		if sequencer == nil || sequencer.Callee == nil {
			continue
		}

		if !isSequencerTransformation(tp, c, sequencer) {
			continue
		}

		loc := scanner.GetErrorRangeForNode(sf, sequencer.Callee)
		if visitedCallees[loc] {
			continue
		}
		visitedCallees[loc] = true

		matches = append(matches, SleepThenEffectToDelayMatch{
			SourceFile: sf,
			Location:   loc,
			Callee:     sequencer.Callee,
		})
	}

	return matches
}

func isDirectEffectSleepCall(tp *typeparser.TypeParser, node *ast.Node) bool {
	if node == nil {
		return false
	}
	node = ast.SkipParentheses(node)
	if node.Kind != ast.KindCallExpression {
		return false
	}
	call := node.AsCallExpression()
	if call == nil || call.Expression == nil {
		return false
	}
	return tp.IsNodeReferenceToEffectModuleApi(call.Expression, "sleep")
}

func isEffectSleepTransformation(tp *typeparser.TypeParser, transformation *typeparser.PipingFlowTransformation) bool {
	if transformation == nil || transformation.Callee == nil {
		return false
	}
	return tp.IsNodeReferenceToEffectModuleApi(transformation.Callee, "sleep")
}

func isSequencerTransformation(tp *typeparser.TypeParser, c *checker.Checker, transformation *typeparser.PipingFlowTransformation) bool {
	if tp == nil || c == nil || transformation == nil || transformation.Callee == nil || len(transformation.Args) == 0 {
		return false
	}
	callee := transformation.Callee
	arg := transformation.Args[0]
	if arg == nil {
		return false
	}

	switch {
	case tp.IsNodeReferenceToEffectModuleApi(callee, "andThen"):
		t := tp.GetTypeAtLocation(arg)
		return sleepContinuationIsEffect(tp, t)

	case tp.IsNodeReferenceToEffectModuleApi(callee, "zipRight"):
		t := tp.GetTypeAtLocation(arg)
		return sleepContinuationIsEffect(tp, t)

	case tp.IsNodeReferenceToEffectModuleApi(callee, "flatMap"):
		return isCallbackIgnoringParam(tp, c, arg)

	default:
		return false
	}
}

func isCallbackIgnoringParam(tp *typeparser.TypeParser, c *checker.Checker, callbackNode *ast.Node) bool {
	if callbackNode == nil {
		return false
	}
	callbackNode = ast.SkipParentheses(callbackNode)
	var params *ast.NodeList
	var body *ast.Node

	switch callbackNode.Kind {
	case ast.KindArrowFunction:
		fn := callbackNode.AsArrowFunction()
		params = fn.Parameters
		body = fn.Body
	case ast.KindFunctionExpression:
		fn := callbackNode.AsFunctionExpression()
		params = fn.Parameters
		body = fn.Body
	default:
		return false
	}

	if body == nil {
		return false
	}

	paramCount := 0
	if params != nil {
		paramCount = len(params.Nodes)
	}

	if paramCount == 0 {
		return true
	}

	if paramCount == 1 {
		paramDecl := params.Nodes[0].AsParameterDeclaration()
		if paramDecl == nil || paramDecl.Name() == nil || paramDecl.DotDotDotToken != nil {
			return false
		}
		nameNode := paramDecl.Name()
		if nameNode.Kind != ast.KindIdentifier {
			return false
		}
		paramSymbol := tp.GetSymbolAtLocation(nameNode)
		if paramSymbol == nil {
			return true
		}
		return !sleepParamIsReferenced(tp, c, paramSymbol, body)
	}

	return false
}

// sleepParamIsReferenced checks if the given parameter symbol is referenced in the node tree.
func sleepParamIsReferenced(tp *typeparser.TypeParser, c *checker.Checker, paramSymbol *ast.Symbol, body *ast.Node) bool {
	var usesParameter func(node *ast.Node) bool
	usesParameter = func(node *ast.Node) bool {
		if node == nil {
			return false
		}
		if node.Kind == ast.KindShorthandPropertyAssignment && c.GetShorthandAssignmentValueSymbol(node) == paramSymbol {
			return true
		}
		if node.Kind == ast.KindIdentifier {
			sym := tp.GetSymbolAtLocation(node)
			if sym != nil && (sym == paramSymbol || checker.Checker_getSymbolIfSameReference(c, sym, paramSymbol) != nil) {
				return true
			}
		}
		return node.ForEachChild(usesParameter)
	}
	return usesParameter(body)
}

// sleepContinuationIsEffect checks whether all constituents of the type (unrolling unions)
// satisfy the Effect variance interface.
func sleepContinuationIsEffect(tp *typeparser.TypeParser, t *checker.Type) bool {
	if tp == nil || t == nil {
		return false
	}
	members := tp.UnrollUnionMembers(t)
	if len(members) == 0 {
		return false
	}
	for _, m := range members {
		if !tp.IsEffectType(m) {
			return false
		}
	}
	return true
}
