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

// EffectSucceedWithVoid suggests using Effect.void instead of Effect.succeed(undefined) or Effect.succeed(void 0).
var EffectSucceedWithVoid = rule.Rule{
	Name:            "effectSucceedWithVoid",
	Group:           "style",
	Description:     "Suggests using Effect.void instead of Effect.succeed(undefined) or Effect.succeed(void 0)",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.Effect_void_represents_the_same_outcome_as_Effect_succeed_undefined_or_Effect_succeed_void_0_effect_effectSucceedWithVoid.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeEffectSucceedWithVoid(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			diags[i] = ctx.NewDiagnostic(m.SourceFile, m.Location, tsdiag.Effect_void_represents_the_same_outcome_as_Effect_succeed_undefined_or_Effect_succeed_void_0_effect_effectSucceedWithVoid, nil)
		}
		return diags
	},
}

// EffectSucceedConstantMatch holds the nodes needed by the effectSucceedWithVoid and
// effectSucceedWithUndefined diagnostics and their quick-fixes.
type EffectSucceedConstantMatch struct {
	SourceFile *ast.SourceFile        // The source file where the diagnostic should be reported
	Location   core.TextRange         // The pre-computed error range for this match
	Flow       *typeparser.PipingFlow // The flow whose subject and first step are replaced
	Member     string                 // The Effect constant that replaces them: "void" or "undefined"
	Rewritable bool                   // False when replacing the step would drop a side effect of the callee
}

// isVoidExpression checks if a node is `undefined`, `void 0`, or a parenthesized version of either.
func isVoidExpression(node *ast.Node) bool {
	// Unwrap parenthesized expressions
	node = ast.SkipParentheses(node)

	// Check for `undefined`
	if node.Kind == ast.KindIdentifier && scanner.GetTextOfNode(node) == "undefined" {
		return true
	}

	// Check for `void 0`
	if node.Kind == ast.KindVoidExpression {
		operand := node.AsVoidExpression().Expression
		if operand.Kind == ast.KindNumericLiteral && scanner.GetTextOfNode(operand) == "0" {
			return true
		}
	}

	return false
}

// AnalyzeEffectSucceedWithVoid finds Effect.succeed(undefined) and Effect.succeed(void 0)
// steps that Effect.void replaces.
func AnalyzeEffectSucceedWithVoid(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile) []EffectSucceedConstantMatch {
	return filterEffectSucceedConstantMatches(analyzeEffectSucceedConstant(tp, sf), "void")
}

func filterEffectSucceedConstantMatches(matches []EffectSucceedConstantMatch, member string) []EffectSucceedConstantMatch {
	var result []EffectSucceedConstantMatch
	for _, match := range matches {
		if match.Member == member {
			result = append(result, match)
		}
	}
	return result
}

// analyzeEffectSucceedConstant finds piping flows whose subject is the constant `undefined`
// or `void 0`, possibly parenthesized, and whose first step is Effect.succeed, as a direct call
// or as a pipe step. The step's success type picks the replacement: void gets Effect.void,
// undefined gets Effect.undefined in Effect v4 and Effect.void in v3, which has no
// Effect.undefined. Any other success type, such as an explicit <number | undefined>, is
// left alone.
func analyzeEffectSucceedConstant(tp *typeparser.TypeParser, sf *ast.SourceFile) []EffectSucceedConstantMatch {
	if tp == nil || sf == nil {
		return nil
	}

	var matches []EffectSucceedConstantMatch
	for _, flow := range tp.PipingFlows(sf, true).All() {
		var member string
		if !flow.MatchesPrefix(
			func(subject typeparser.PipingFlowSubject) bool { return isUndefinedConstant(tp, subject.Node()) },
			func(transformation typeparser.PipingFlowTransformation) bool {
				if !isReplaceableSucceedStep(transformation.Kind()) || transformation.Callee() == nil ||
					transformation.Args().Len() != 0 ||
					!tp.IsNodeReferenceToEffectModuleApi(unwrapZeroDurationExpression(transformation.Callee()), "succeed") {
					return false
				}
				member = effectSucceedConstantMember(tp, tp.StrictEffectType(transformation.OutType()))
				return member != ""
			},
		) {
			continue
		}

		callee := flow.Transformations().At(0).Callee()
		matches = append(matches, EffectSucceedConstantMatch{
			SourceFile: sf,
			Location:   scanner.GetErrorRangeForNode(sf, callee),
			Flow:       flow,
			Member:     member,
			Rewritable: canRewriteSucceedCallee(tp, callee),
		})
	}
	return matches
}

// isUndefinedConstant checks if a node is `undefined` or `void 0`, possibly parenthesized,
// where `undefined` must resolve to the global.
func isUndefinedConstant(tp *typeparser.TypeParser, node *ast.Node) bool {
	if node == nil || !isVoidExpression(node) {
		return false
	}
	node = ast.SkipParentheses(node)
	// The stability check accepts `undefined` only when it is the global.
	return node.Kind != ast.KindIdentifier || tp.IsExpressionValueStableAtLocation(node, node)
}

// effectSucceedConstantMember returns the Effect constant with the same type as an
// Effect.succeed result, or "" when there is none.
func effectSucceedConstantMember(tp *typeparser.TypeParser, effect *typeparser.Effect) string {
	if effect == nil || effect.A() == nil {
		return ""
	}
	switch {
	case effect.A().Flags()&checker.TypeFlagsVoid != 0:
		return "void"
	case effect.A().Flags()&checker.TypeFlagsUndefined != 0:
		if tp.SupportedEffectVersion() == typeparser.EffectMajorV4 {
			return "undefined"
		}
		return "void"
	}
	return ""
}

// isReplaceableSucceedStep reports whether an Effect.succeed step of this kind applies to the
// flow subject: a direct call or a pipe step.
func isReplaceableSucceedStep(kind typeparser.TransformationKind) bool {
	return kind == typeparser.TransformationKindCall || kind == typeparser.TransformationKindPipe
}

// canRewriteSucceedCallee reports whether dropping the callee keeps the program's behaviour:
// an identifier, or a property access on an identifier or an imported module.
func canRewriteSucceedCallee(tp *typeparser.TypeParser, expr *ast.Node) bool {
	callee := unwrapZeroDurationExpression(expr)
	switch callee.Kind {
	case ast.KindIdentifier:
		return true
	case ast.KindPropertyAccessExpression:
		if callee.Flags&ast.NodeFlagsOptionalChain != 0 {
			return false
		}
		receiver := callee.AsPropertyAccessExpression().Expression
		return unwrapZeroDurationExpression(receiver).Kind == ast.KindIdentifier || isImportedModuleReceiver(tp, receiver)
	}
	return false
}
