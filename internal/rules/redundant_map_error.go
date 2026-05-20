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

var RedundantMapError = rule.Rule{
	Name:            "redundantMapError",
	Group:           "style",
	Description:     "Suggests hoisting a repeated trailing Effect.mapError from every yield in an Effect generator",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.This_generator_applies_the_same_trailing_Effect_mapError_to_every_yielded_effect_Hoist_it_once_at_the_generator_result_Colon_Effect_gen_pipe_Effect_mapError_or_the_trailing_pipe_arguments_of_generator_form_Effect_fn_effect_redundantMapError.Code(),
		tsdiag.This_repeated_Effect_mapError_site_participates_in_the_hoistable_generator_wide_mapping_effect_redundantMapError.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeRedundantGeneratorMapError(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diags[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.This_generator_applies_the_same_trailing_Effect_mapError_to_every_yielded_effect_Hoist_it_once_at_the_generator_result_Colon_Effect_gen_pipe_Effect_mapError_or_the_trailing_pipe_arguments_of_generator_form_Effect_fn_effect_redundantMapError,
				redundantMapErrorRelatedInformation(ctx, match),
			)
		}
		return diags
	},
}

type RedundantGeneratorMapErrorMatch struct {
	SourceFile        *ast.SourceFile
	Location          core.TextRange
	GeneratorCallNode *ast.Node
	GeneratorFunction *ast.FunctionExpression
	MapErrorNodes     []*ast.Node
	MapperNode        *ast.Node
	YieldExpressions  []*ast.Node
}

type redundantMapErrorCandidate struct {
	yieldExpression *ast.Node
	mapErrorNode    *ast.Node
	mapperNode      *ast.Node
	mapperText      string
}

func AnalyzeRedundantGeneratorMapError(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []RedundantGeneratorMapErrorMatch {
	if tp == nil || c == nil || sf == nil {
		return nil
	}

	generatorCalls := make(map[*ast.FunctionExpression]*ast.Node)
	candidatesByGenerator := make(map[*ast.FunctionExpression]map[*ast.Node]redundantMapErrorCandidate)

	for _, flow := range tp.PipingFlows(sf, true) {
		candidate, genFn, genCallNode, ok := analyzeRedundantMapErrorFlowCandidate(tp, sf, flow)
		if !ok {
			continue
		}
		generatorCalls[genFn] = genCallNode
		if candidatesByGenerator[genFn] == nil {
			candidatesByGenerator[genFn] = make(map[*ast.Node]redundantMapErrorCandidate)
		}
		candidatesByGenerator[genFn][candidate.yieldExpression] = candidate
	}

	var matches []RedundantGeneratorMapErrorMatch
	for genFn, candidates := range candidatesByGenerator {
		if len(candidates) < 2 {
			continue
		}
		if match, ok := analyzeRedundantGeneratorMapErrorCandidate(tp, c, sf, generatorCalls[genFn], genFn, candidates); ok {
			matches = append(matches, match)
		}
	}

	return matches
}

func analyzeRedundantMapErrorFlowCandidate(tp *typeparser.TypeParser, sf *ast.SourceFile, flow *typeparser.PipingFlow) (redundantMapErrorCandidate, *ast.FunctionExpression, *ast.Node, bool) {
	if tp == nil || sf == nil || flow == nil || flow.Node == nil || len(flow.Transformations) == 0 {
		return redundantMapErrorCandidate{}, nil, nil, false
	}

	lastTransform := flow.Transformations[len(flow.Transformations)-1]
	if lastTransform.Node == nil || lastTransform.Callee == nil || len(lastTransform.Args) == 0 {
		return redundantMapErrorCandidate{}, nil, nil, false
	}
	if !tp.IsNodeReferenceToEffectModuleApi(lastTransform.Callee, "mapError") {
		return redundantMapErrorCandidate{}, nil, nil, false
	}

	yieldExpr := enclosingYieldExpression(flow.Node)
	if yieldExpr == nil || yieldExpr.Expression == nil || yieldExpr.Expression != flow.Node || yieldExpr.AsteriskToken == nil {
		return redundantMapErrorCandidate{}, nil, nil, false
	}
	if tp.GetEffectContextFlags(yieldExpr.AsNode())&typeparser.EffectContextFlagCanYieldEffect == 0 {
		return redundantMapErrorCandidate{}, nil, nil, false
	}

	genFn := tp.GetEffectYieldGeneratorFunction(yieldExpr.AsNode())
	if genFn == nil {
		return redundantMapErrorCandidate{}, nil, nil, false
	}

	genCallNode := enclosingGeneratorCall(genFn.AsNode())
	if genCallNode == nil {
		return redundantMapErrorCandidate{}, nil, nil, false
	}

	mapperNode := unwrapTransparentExpression(lastTransform.Args[0])
	mapperText := nodeSourceText(sf, mapperNode)
	if mapperNode == nil || mapperText == "" {
		return redundantMapErrorCandidate{}, nil, nil, false
	}

	return redundantMapErrorCandidate{
		yieldExpression: yieldExpr.AsNode(),
		mapErrorNode:    lastTransform.Node,
		mapperNode:      mapperNode,
		mapperText:      mapperText,
	}, genFn, genCallNode, true
}

func analyzeRedundantGeneratorMapErrorCandidate(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile, generatorCallNode *ast.Node, generatorFunction *ast.FunctionExpression, candidates map[*ast.Node]redundantMapErrorCandidate) (RedundantGeneratorMapErrorMatch, bool) {
	if tp == nil || c == nil || sf == nil || generatorCallNode == nil || generatorFunction == nil {
		return RedundantGeneratorMapErrorMatch{}, false
	}

	generatorBody := generatorFunction.Body
	if generatorBody == nil {
		return RedundantGeneratorMapErrorMatch{}, false
	}

	var yieldExpressions []*ast.Node
	var representativeMapError *ast.Node
	var mapErrorNodes []*ast.Node
	var representativeMapper *ast.Node
	var representativeText string
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
		if !mapperCanBeHoisted(c, generatorFunction.AsNode(), generatorCallNode, candidate.mapperNode) {
			validYieldCount = -1
			return true
		}

		if representativeMapError == nil {
			representativeMapError = candidate.mapErrorNode
			representativeMapper = candidate.mapperNode
			representativeText = candidate.mapperText
		} else if candidate.mapperText != representativeText {
			validYieldCount = -1
			return true
		}

		yieldExpressions = append(yieldExpressions, expr)
		mapErrorNodes = append(mapErrorNodes, candidate.mapErrorNode)
		validYieldCount++
		return false
	})

	if validYieldCount < 2 || validYieldCount != len(candidates) || representativeMapError == nil || representativeMapper == nil {
		return RedundantGeneratorMapErrorMatch{}, false
	}

	return RedundantGeneratorMapErrorMatch{
		SourceFile:        sf,
		Location:          generatorFunctionKeywordRange(sf, generatorFunction),
		GeneratorCallNode: generatorCallNode,
		GeneratorFunction: generatorFunction,
		MapErrorNodes:     mapErrorNodes,
		MapperNode:        representativeMapper,
		YieldExpressions:  yieldExpressions,
	}, true
}

func redundantMapErrorRelatedInformation(ctx *rule.Context, match RedundantGeneratorMapErrorMatch) []*ast.Diagnostic {
	if ctx == nil || len(match.MapErrorNodes) == 0 {
		return nil
	}

	related := make([]*ast.Diagnostic, 0, len(match.MapErrorNodes))
	for _, node := range match.MapErrorNodes {
		if node == nil {
			continue
		}
		related = append(related, ctx.NewDiagnostic(
			match.SourceFile,
			scanner.GetErrorRangeForNode(match.SourceFile, node),
			tsdiag.This_repeated_Effect_mapError_site_participates_in_the_hoistable_generator_wide_mapping_effect_redundantMapError,
			nil,
		))
	}
	return related
}

func enclosingYieldExpression(node *ast.Node) *ast.YieldExpression {
	ancestor := ast.FindAncestor(node, func(current *ast.Node) bool {
		return current != nil && current.Kind == ast.KindYieldExpression
	})
	if ancestor == nil {
		return nil
	}
	return ancestor.AsYieldExpression()
}

func enclosingGeneratorCall(node *ast.Node) *ast.Node {
	for current := node; current != nil; current = current.Parent {
		if current.Kind != ast.KindCallExpression {
			continue
		}
		return current
	}
	return nil
}

func mapperCanBeHoisted(c *checker.Checker, generatorFunctionNode *ast.Node, hoistLocation *ast.Node, mapperNode *ast.Node) bool {
	if c == nil || generatorFunctionNode == nil || hoistLocation == nil || mapperNode == nil {
		return false
	}

	var canHoist = true
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil || !canHoist {
			return canHoist
		}
		if node.Kind == ast.KindIdentifier && isHoistSensitiveValueReference(node) {
			symbol := hoistSensitiveReferenceSymbol(c, node)
			if symbol != nil {
				if allSymbolDeclarationsInsideNode(symbol, mapperNode) {
					node.ForEachChild(walk)
					return !canHoist
				}

				if !allSymbolDeclarationsOutsideNode(symbol, generatorFunctionNode) {
					canHoist = false
					return true
				}

				if !checker.Checker_isPastLastAssignment(c, symbol, hoistLocation) {
					canHoist = false
					return true
				}
			}
		}
		node.ForEachChild(walk)
		return !canHoist
	}

	walk(mapperNode)
	return canHoist
}

func allSymbolDeclarationsInsideNode(symbol *ast.Symbol, node *ast.Node) bool {
	if symbol == nil || node == nil || len(symbol.Declarations) == 0 {
		return false
	}
	for _, declaration := range symbol.Declarations {
		if declaration == nil || !ast.IsNodeDescendantOf(declaration, node) {
			return false
		}
	}
	return true
}

func allSymbolDeclarationsOutsideNode(symbol *ast.Symbol, node *ast.Node) bool {
	if symbol == nil || node == nil {
		return false
	}
	for _, declaration := range symbol.Declarations {
		if declaration != nil && ast.IsNodeDescendantOf(declaration, node) {
			return false
		}
	}
	return true
}

func hoistSensitiveReferenceSymbol(c *checker.Checker, node *ast.Node) *ast.Symbol {
	if c == nil || node == nil {
		return nil
	}
	if node.Parent != nil && node.Parent.Kind == ast.KindShorthandPropertyAssignment {
		if symbol := c.GetShorthandAssignmentValueSymbol(node.Parent); symbol != nil {
			return symbol
		}
	}
	return c.GetSymbolAtLocation(node)
}

func generatorFunctionKeywordRange(sf *ast.SourceFile, generatorFunction *ast.FunctionExpression) core.TextRange {
	if sf == nil || generatorFunction == nil || generatorFunction.AsteriskToken == nil {
		return scanner.GetErrorRangeForNode(sf, generatorFunction.AsNode())
	}
	start := scanner.GetTokenPosOfNode(generatorFunction.AsNode(), sf, false)
	end := generatorFunction.AsteriskToken.End()
	if start < 0 || end < start {
		return scanner.GetErrorRangeForNode(sf, generatorFunction.AsNode())
	}
	return core.NewTextRange(start, end)
}

func isHoistSensitiveValueReference(node *ast.Node) bool {
	if node == nil || node.Kind != ast.KindIdentifier || ast.IsDeclarationNameOrImportPropertyName(node) || ast.IsValidTypeOnlyAliasUseSite(node) {
		return false
	}

	parent := node.Parent
	if parent == nil {
		return true
	}

	switch parent.Kind {
	case ast.KindPropertyAccessExpression:
		prop := parent.AsPropertyAccessExpression()
		return prop == nil || prop.Name() != node
	case ast.KindQualifiedName:
		qn := parent.AsQualifiedName()
		return qn == nil || qn.Right != node
	case ast.KindPropertyAssignment:
		assignment := parent.AsPropertyAssignment()
		if assignment != nil && assignment.Name() == node && assignment.Initializer != nil && assignment.Initializer != node {
			return false
		}
	}

	return true
}

func unwrapTransparentExpression(node *ast.Node) *ast.Node {
	for node != nil {
		switch node.Kind {
		case ast.KindParenthesizedExpression, ast.KindSatisfiesExpression, ast.KindAsExpression, ast.KindNonNullExpression, ast.KindTypeAssertionExpression:
			node = node.Expression()
		default:
			return node
		}
	}
	return nil
}

func nodeSourceText(sf *ast.SourceFile, node *ast.Node) string {
	if sf == nil || node == nil {
		return ""
	}
	text := sf.Text()
	pos := scanner.GetTokenPosOfNode(node, sf, false)
	end := node.End()
	if pos < 0 || end < pos || end > len(text) {
		return ""
	}
	return text[pos:end]
}
