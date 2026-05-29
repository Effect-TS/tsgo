package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

var RedundantOrDie = rule.Rule{
	Name:            "redundantOrDie",
	Group:           "style",
	Description:     "Suggests hoisting a repeated trailing Effect.orDie from every yield in an Effect generator",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.This_generator_applies_Effect_orDie_to_every_yielded_effect_Hoist_it_once_to_the_generator_result_Colon_Effect_gen_pipe_Effect_orDie_or_Effect_fn_function_Asterisk_Effect_orDie_effect_redundantOrDie.Code(),
		tsdiag.This_repeated_Effect_orDie_site_participates_in_the_hoistable_generator_wide_orDie_effect_redundantOrDie.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeRedundantGeneratorOrDie(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diags[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.This_generator_applies_Effect_orDie_to_every_yielded_effect_Hoist_it_once_to_the_generator_result_Colon_Effect_gen_pipe_Effect_orDie_or_Effect_fn_function_Asterisk_Effect_orDie_effect_redundantOrDie,
				redundantOrDieRelatedInformation(ctx, match),
			)
		}
		return diags
	},
}

type RedundantGeneratorOrDieMatch struct {
	SourceFile        *ast.SourceFile
	Location          core.TextRange
	GeneratorCallNode *ast.Node
	GeneratorFunction *ast.FunctionExpression
	OrDieNodes        []*ast.Node
	YieldExpressions  []*ast.Node
}

type redundantOrDieCandidate struct {
	yieldExpression *ast.Node
	orDieNode       *ast.Node
}

func AnalyzeRedundantGeneratorOrDie(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []RedundantGeneratorOrDieMatch {
	if tp == nil || c == nil || sf == nil {
		return nil
	}

	generatorCalls := make(map[*ast.FunctionExpression]*ast.Node)
	candidatesByGenerator := make(map[*ast.FunctionExpression]map[*ast.Node]redundantOrDieCandidate)

	for _, flow := range tp.PipingFlows(sf, true) {
		candidate, genFn, genCallNode, ok := analyzeRedundantOrDieFlowCandidate(tp, sf, flow)
		if !ok {
			continue
		}
		generatorCalls[genFn] = genCallNode
		if candidatesByGenerator[genFn] == nil {
			candidatesByGenerator[genFn] = make(map[*ast.Node]redundantOrDieCandidate)
		}
		candidatesByGenerator[genFn][candidate.yieldExpression] = candidate
	}

	var matches []RedundantGeneratorOrDieMatch
	for genFn, candidates := range candidatesByGenerator {
		if len(candidates) < 2 {
			continue
		}
		if match, ok := analyzeRedundantGeneratorOrDieCandidate(tp, sf, generatorCalls[genFn], genFn, candidates); ok {
			matches = append(matches, match)
		}
	}

	return matches
}

func analyzeRedundantOrDieFlowCandidate(tp *typeparser.TypeParser, sf *ast.SourceFile, flow *typeparser.PipingFlow) (redundantOrDieCandidate, *ast.FunctionExpression, *ast.Node, bool) {
	if tp == nil || sf == nil || flow == nil || flow.Node == nil || len(flow.Transformations) == 0 {
		return redundantOrDieCandidate{}, nil, nil, false
	}

	lastTransform := flow.Transformations[len(flow.Transformations)-1]
	if lastTransform.Node == nil || lastTransform.Callee == nil {
		return redundantOrDieCandidate{}, nil, nil, false
	}
	if !tp.IsNodeReferenceToEffectModuleApi(lastTransform.Callee, "orDie") {
		return redundantOrDieCandidate{}, nil, nil, false
	}

	yieldExpr := enclosingYieldExpression(flow.Node)
	if yieldExpr == nil || yieldExpr.Expression == nil || yieldExpr.Expression != flow.Node || yieldExpr.AsteriskToken == nil {
		return redundantOrDieCandidate{}, nil, nil, false
	}
	if tp.GetEffectContextFlags(yieldExpr.AsNode())&typeparser.EffectContextFlagCanYieldEffect == 0 {
		return redundantOrDieCandidate{}, nil, nil, false
	}

	genFn := tp.GetEffectYieldGeneratorFunction(yieldExpr.AsNode())
	if genFn == nil {
		return redundantOrDieCandidate{}, nil, nil, false
	}

	genCallNode := enclosingGeneratorCall(genFn.AsNode())
	if genCallNode == nil {
		return redundantOrDieCandidate{}, nil, nil, false
	}

	return redundantOrDieCandidate{
		yieldExpression: yieldExpr.AsNode(),
		orDieNode:       lastTransform.Node,
	}, genFn, genCallNode, true
}

func analyzeRedundantGeneratorOrDieCandidate(tp *typeparser.TypeParser, sf *ast.SourceFile, generatorCallNode *ast.Node, generatorFunction *ast.FunctionExpression, candidates map[*ast.Node]redundantOrDieCandidate) (RedundantGeneratorOrDieMatch, bool) {
	if tp == nil || sf == nil || generatorCallNode == nil || generatorFunction == nil {
		return RedundantGeneratorOrDieMatch{}, false
	}

	generatorBody := generatorFunction.Body
	if generatorBody == nil {
		return RedundantGeneratorOrDieMatch{}, false
	}

	var yieldExpressions []*ast.Node
	var orDieNodes []*ast.Node
	validYieldCount := 0

	checker.ForEachYieldExpression(generatorBody.AsNode(), func(expr *ast.Node) bool {
		if expr == nil || expr.Kind != ast.KindYieldExpression {
			return false
		}
		if tp.GetEffectContextFlags(expr)&typeparser.EffectContextFlagCanYieldEffect == 0 || tp.GetEffectYieldGeneratorFunction(expr) != generatorFunction {
			return false
		}

		yieldExpr := expr.AsYieldExpression()
		if yieldExpr == nil || yieldExpr.AsteriskToken == nil || yieldExpr.Expression == nil {
			return false
		}

		candidate, ok := candidates[expr]
		if !ok {
			validYieldCount = -1
			return true
		}

		yieldExpressions = append(yieldExpressions, expr)
		orDieNodes = append(orDieNodes, candidate.orDieNode)
		validYieldCount++
		return false
	})

	if validYieldCount < 2 || validYieldCount != len(candidates) || len(orDieNodes) == 0 {
		return RedundantGeneratorOrDieMatch{}, false
	}

	return RedundantGeneratorOrDieMatch{
		SourceFile:        sf,
		Location:          generatorFunctionKeywordRange(sf, generatorFunction),
		GeneratorCallNode: generatorCallNode,
		GeneratorFunction: generatorFunction,
		OrDieNodes:        orDieNodes,
		YieldExpressions:  yieldExpressions,
	}, true
}

func redundantOrDieRelatedInformation(ctx *rule.Context, match RedundantGeneratorOrDieMatch) []*ast.Diagnostic {
	if ctx == nil || len(match.OrDieNodes) == 0 {
		return nil
	}

	related := make([]*ast.Diagnostic, 0, len(match.OrDieNodes))
	for _, node := range match.OrDieNodes {
		if node == nil {
			continue
		}
		related = append(related, ctx.NewDiagnostic(
			match.SourceFile,
			scanner.GetErrorRangeForNode(match.SourceFile, node),
			tsdiag.This_repeated_Effect_orDie_site_participates_in_the_hoistable_generator_wide_orDie_effect_redundantOrDie,
			nil,
		))
	}
	return related
}
