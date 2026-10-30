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

var EmptyCallbackToNever = rule.Rule{
	Name:            "emptyCallbackToNever",
	Group:           "style",
	Description:     "Suggests Effect.never instead of Effect.callback with an empty register",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v4"},
	Codes: []int32{
		tsdiag.This_Effect_callback_register_is_empty_so_the_effect_never_completes_Use_Effect_never_instead_effect_emptyCallbackToNever.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeEmptyCallbackToNever(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diags[i] = ctx.NewDiagnostic(match.SourceFile, match.Location,
				tsdiag.This_Effect_callback_register_is_empty_so_the_effect_never_completes_Use_Effect_never_instead_effect_emptyCallbackToNever, nil)
		}
		return diags
	},
}

type EmptyCallbackToNeverMatch struct {
	SourceFile *ast.SourceFile
	Location   core.TextRange
	// Flow has the register as its subject and Effect.callback as its first step;
	// the fix replaces that prefix.
	Flow *typeparser.PipingFlow
	// EffectModule is the callee's own module receiver, or nil when no fix is offered.
	EffectModule *ast.Node
}

// AnalyzeEmptyCallbackToNever finds v4 piping flows that apply Effect.callback to a
// function literal with an empty body, where the explicit type arguments, if any, are
// all never. The register is the subject of its flow, whether it is written as
// Effect.callback(register) or piped into a bare Effect.callback step.
func AnalyzeEmptyCallbackToNever(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile) []EmptyCallbackToNeverMatch {
	if tp == nil || sf == nil || tp.SupportedEffectVersion() != typeparser.EffectMajorV4 {
		return nil
	}

	var matches []EmptyCallbackToNeverMatch
	for _, flow := range tp.PipingFlows(sf, true).All() {
		var callee *ast.Node
		if !flow.MatchesPrefix(
			func(subject typeparser.PipingFlowSubject) bool { return isEmptyRegisterLiteral(subject.Node()) },
			func(transformation typeparser.PipingFlowTransformation) bool {
				callee = emptyCallbackCallee(transformation)
				return callee != nil && tp.IsNodeReferenceToEffectModuleApi(callee, "callback")
			},
		) {
			continue
		}
		var receiver *ast.Node
		if callee.Kind == ast.KindPropertyAccessExpression {
			candidate := callee.AsPropertyAccessExpression().Expression
			if isImportedModuleReceiver(tp, candidate) && tp.IsExpressionEffectModule(unwrapZeroDurationExpression(candidate)) {
				receiver = candidate
			}
		}
		matches = append(matches, EmptyCallbackToNeverMatch{
			SourceFile:   sf,
			Location:     scanner.GetErrorRangeForNode(sf, flow.Transformations().At(0).Callee()),
			Flow:         flow,
			EffectModule: receiver,
		})
	}
	return matches
}

// emptyCallbackCallee returns the callee of a bare step, unwrapped from parentheses,
// assertions and instantiation expressions, or nil when the step is an optional call,
// takes arguments of its own, or has an explicit type argument other than never.
func emptyCallbackCallee(transformation typeparser.PipingFlowTransformation) *ast.Node {
	callee := transformation.Callee()
	if callee == nil || transformation.Args().Len() != 0 {
		return nil
	}
	if call := callee.Parent; call != nil && ast.IsCallExpression(call) && call.AsCallExpression().Expression == callee &&
		call.Flags&ast.NodeFlagsOptionalChain != 0 {
		return nil
	}
	typeArguments := []*ast.NodeList{transformation.TypeArguments()}
	callee = unwrapZeroDurationExpression(callee)
	for callee.Kind == ast.KindExpressionWithTypeArguments {
		instantiation := callee.AsExpressionWithTypeArguments()
		typeArguments = append(typeArguments, instantiation.TypeArguments)
		callee = unwrapZeroDurationExpression(instantiation.Expression)
	}
	if callee.Flags&ast.NodeFlagsOptionalChain != 0 {
		return nil
	}
	for _, list := range typeArguments {
		if list == nil {
			continue
		}
		for _, typeArgument := range list.Nodes {
			if ast.SkipTypeParentheses(typeArgument).Kind != ast.KindNeverKeyword {
				return nil
			}
		}
	}
	return callee
}

// isEmptyRegisterLiteral reports whether node is a non-async, non-generator arrow or function
// expression with a statement-free block body and plain parameters without defaults.
func isEmptyRegisterLiteral(node *ast.Node) bool {
	if node == nil {
		return false
	}
	switch node.Kind {
	case ast.KindArrowFunction:
	case ast.KindFunctionExpression:
		if node.AsFunctionExpression().AsteriskToken != nil {
			return false
		}
	default:
		return false
	}
	if ast.HasSyntacticModifier(node, ast.ModifierFlagsAsync) {
		return false
	}
	body := typeparser.GetFunctionLikeBody(node)
	if body == nil || body.Kind != ast.KindBlock {
		return false
	}
	if statements := body.AsBlock().Statements; statements != nil && len(statements.Nodes) != 0 {
		return false
	}
	if parameters := typeparser.GetFunctionLikeParameters(node); parameters != nil {
		for _, parameter := range parameters.Nodes {
			declaration := parameter.AsParameterDeclaration()
			if declaration.Initializer != nil || ast.IsBindingPattern(declaration.Name()) {
				return false
			}
		}
	}
	return true
}
