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

// CatchAllDieToOrDie suggests using Effect.orDie instead of a catch-all
// handler that forwards the typed failure unchanged to Effect.die.
var CatchAllDieToOrDie = rule.Rule{
	Name:            "catchAllDieToOrDie",
	Group:           "style",
	Description:     "Suggests using Effect.orDie instead of Effect.catch or Effect.catchAll with an identity-forwarding Effect.die handler",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Effect_orDie_expresses_escalating_every_typed_failure_into_a_defect_more_directly_than_Effect_0_with_an_identity_forwarding_Effect_die_handler_effect_catchAllDieToOrDie.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeCatchAllDieToOrDie(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diagnostics := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diagnostics[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.Effect_orDie_expresses_escalating_every_typed_failure_into_a_defect_more_directly_than_Effect_0_with_an_identity_forwarding_Effect_die_handler_effect_catchAllDieToOrDie,
				nil,
				match.CatchMethodName,
			)
		}
		return diagnostics
	},
}

// CatchAllDieToOrDieMatch holds the nodes needed by the diagnostic and fix.
type CatchAllDieToOrDieMatch struct {
	SourceFile        *ast.SourceFile
	Location          core.TextRange
	Transformation    *ast.Node
	EffectModule      *ast.Node
	Input             *ast.Node
	CatchMethodName   string
	IsDataApplication bool
	HasTypeArguments  bool
}

// AnalyzeCatchAllDieToOrDie finds Effect.catch/catchAll transformations whose
// handler is Effect.die or forwards its sole argument unchanged to Effect.die.
func AnalyzeCatchAllDieToOrDie(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile) []CatchAllDieToOrDieMatch {
	if tp == nil || sf == nil {
		return nil
	}

	var matches []CatchAllDieToOrDieMatch
	seen := make(map[*ast.Node]struct{})
	for _, flow := range tp.PipingFlows(sf, true) {
		inputNode := flow.Subject.Node
		inputType := flow.Subject.OutType
		for i := range flow.Transformations {
			transformation := &flow.Transformations[i]
			callee, args, isCurriedApplication := catchAllDieTransformationCall(transformation)
			methodName, effectModule, ok := catchAllDieMethod(tp, callee)
			if ok && len(args) == 1 && tp.IsNodeReferenceToEffectModuleApi(tp.UnwrapIdentityForwarder(args[0]), "die") {
				inputEffect := tp.StrictEffectType(inputType, callee)
				if inputEffect != nil && inputEffect.E != nil && inputEffect.E.Flags()&checker.TypeFlagsNever == 0 {
					if _, duplicate := seen[transformation.Node]; !duplicate {
						seen[transformation.Node] = struct{}{}
						isDataApplication := transformation.Kind == typeparser.TransformationKindDataFirst ||
							transformation.Kind == typeparser.TransformationKindDataLast || isCurriedApplication
						matches = append(matches, CatchAllDieToOrDieMatch{
							SourceFile:        sf,
							Location:          scanner.GetErrorRangeForNode(sf, callee),
							Transformation:    transformation.Node,
							EffectModule:      effectModule,
							Input:             inputNode,
							CatchMethodName:   methodName,
							IsDataApplication: isDataApplication,
							HasTypeArguments:  catchAllDieHasTypeArguments(transformation),
						})
					}
				}
			}

			inputNode = transformation.Node
			inputType = transformation.OutType
		}
	}

	return matches
}

func catchAllDieTransformationCall(transformation *typeparser.PipingFlowTransformation) (callee *ast.Node, args []*ast.Node, isCurriedApplication bool) {
	if transformation == nil {
		return nil, nil, false
	}
	callee = transformation.Callee
	args = transformation.Args
	if len(args) != 0 || callee == nil || callee.Kind != ast.KindCallExpression {
		return callee, args, false
	}
	call := callee.AsCallExpression()
	if call == nil || call.Expression == nil || call.Arguments == nil {
		return callee, args, false
	}
	return call.Expression, call.Arguments.Nodes, true
}

func catchAllDieMethod(tp *typeparser.TypeParser, callee *ast.Node) (name string, effectModule *ast.Node, ok bool) {
	if callee == nil || callee.Kind != ast.KindPropertyAccessExpression {
		return "", nil, false
	}
	access := callee.AsPropertyAccessExpression()
	if access == nil || access.Name() == nil || access.Expression == nil {
		return "", nil, false
	}

	name = access.Name().Text()
	if name != "catch" && name != "catchAll" {
		return "", nil, false
	}
	if !tp.IsNodeReferenceToEffectModuleApi(callee, name) {
		return "", nil, false
	}
	return name, access.Expression, true
}

func catchAllDieHasTypeArguments(transformation *typeparser.PipingFlowTransformation) bool {
	if transformation == nil {
		return false
	}
	for _, node := range []*ast.Node{transformation.Node, transformation.Callee} {
		if node == nil || node.Kind != ast.KindCallExpression {
			continue
		}
		call := node.AsCallExpression()
		if call != nil && call.TypeArguments != nil && len(call.TypeArguments.Nodes) > 0 {
			return true
		}
	}
	return false
}
