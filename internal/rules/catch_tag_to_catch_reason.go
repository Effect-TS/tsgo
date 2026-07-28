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

var CatchTagToCatchReason = rule.Rule{
	Name:            "catchTagToCatchReason",
	Group:           "style",
	Description:     "Suggests Effect.catchReason or Effect.catchReasons instead of branching on reason._tag inside Effect.catchTag handlers",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v4"},
	Codes: []int32{
		tsdiag.Branching_on_0_reason_tag_inside_Effect_1_hand_rolls_reason_dispatch_use_Effect_catchReason_or_Effect_catchReasons_which_re_fail_unmatched_reasons_automatically_effect_catchTagToCatchReason.Code(),
		tsdiag.Returning_a_successful_Effect_for_unmatched_0_reason_tag_values_inside_Effect_1_silently_swallows_unrelated_reasons_use_Effect_catchReason_or_Effect_catchReasons_to_re_fail_them_automatically_effect_catchTagToCatchReason.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeCatchTagToCatchReason(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diagnostics := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			message := tsdiag.Branching_on_0_reason_tag_inside_Effect_1_hand_rolls_reason_dispatch_use_Effect_catchReason_or_Effect_catchReasons_which_re_fail_unmatched_reasons_automatically_effect_catchTagToCatchReason
			if match.SwallowsUnrelatedReasons {
				message = tsdiag.Returning_a_successful_Effect_for_unmatched_0_reason_tag_values_inside_Effect_1_silently_swallows_unrelated_reasons_use_Effect_catchReason_or_Effect_catchReasons_to_re_fail_them_automatically_effect_catchTagToCatchReason
			}
			diagnostics[i] = ctx.NewDiagnostic(match.SourceFile, match.Location, message, nil, match.ParameterName, match.CatchMethodName)
		}
		return diagnostics
	},
}

type CatchTagToCatchReasonBranch struct {
	ReasonTag        string
	ReturnExpression *ast.Node
}

type CatchTagToCatchReasonMatch struct {
	SourceFile               *ast.SourceFile
	Location                 core.TextRange
	CallNode                 *ast.Node
	Callee                   *ast.Node
	OuterTag                 *ast.Node
	ParameterName            string
	CatchMethodName          string
	Branches                 []CatchTagToCatchReasonBranch
	SwallowsUnrelatedReasons bool
	CanFix                   bool
}

type catchTagToCatchReasonHandler struct {
	parameterName            string
	branches                 []CatchTagToCatchReasonBranch
	swallowsUnrelatedReasons bool
	canFix                   bool
}

type catchTagToCatchReasonControlFlow struct {
	branches      []CatchTagToCatchReasonBranch
	fallback      *ast.Node
	fallbackParam *ast.Node
	dispatchRefs  map[*ast.Node]struct{}
}

// AnalyzeCatchTagToCatchReason finds canonical reason-tag dispatch inside exact
// Effect.catchTag and Effect.catchTags transformations.
func AnalyzeCatchTagToCatchReason(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []CatchTagToCatchReasonMatch {
	if tp == nil || c == nil || sf == nil {
		return nil
	}

	var matches []CatchTagToCatchReasonMatch
	seen := make(map[*ast.Node]struct{})
	for _, flow := range tp.PipingFlows(sf, true) {
		for i := range flow.Transformations {
			transformation := &flow.Transformations[i]
			if transformation.Node == nil || transformation.Callee == nil {
				continue
			}
			if _, ok := seen[transformation.Node]; ok {
				continue
			}

			switch {
			case tp.IsNodeReferenceToEffectModuleApi(transformation.Callee, "catchTag"):
				match, ok := analyzeCatchTagTransformation(tp, c, sf, transformation)
				if ok {
					seen[transformation.Node] = struct{}{}
					matches = append(matches, match)
				}
			case tp.IsNodeReferenceToEffectModuleApi(transformation.Callee, "catchTags"):
				match, ok := analyzeCatchTagsTransformation(tp, c, sf, transformation)
				if ok {
					seen[transformation.Node] = struct{}{}
					matches = append(matches, match)
				}
			}
		}
	}

	return matches
}

func analyzeCatchTagTransformation(
	tp *typeparser.TypeParser,
	c *checker.Checker,
	sf *ast.SourceFile,
	transformation *typeparser.PipingFlowTransformation,
) (CatchTagToCatchReasonMatch, bool) {
	if transformation == nil || len(transformation.Args) != 2 {
		return CatchTagToCatchReasonMatch{}, false
	}

	outerTag := unwrapTransparentExpression(transformation.Args[0])
	if outerTag == nil || !ast.IsStringLiteral(outerTag) {
		return CatchTagToCatchReasonMatch{}, false
	}

	handler, ok := analyzeCatchTagToCatchReasonHandler(tp, c, transformation.Args[1])
	if !ok || !hasCatchReasonApi(tp, c, transformation.Callee, len(handler.branches)) {
		return CatchTagToCatchReasonMatch{}, false
	}

	canFix := handler.canFix && transformationCallHasExactArgs(transformation)
	return CatchTagToCatchReasonMatch{
		SourceFile:               sf,
		Location:                 scanner.GetErrorRangeForNode(sf, transformation.Node),
		CallNode:                 transformation.Node,
		Callee:                   transformation.Callee,
		OuterTag:                 outerTag,
		ParameterName:            handler.parameterName,
		CatchMethodName:          "catchTag",
		Branches:                 handler.branches,
		SwallowsUnrelatedReasons: handler.swallowsUnrelatedReasons,
		CanFix:                   canFix,
	}, true
}

func analyzeCatchTagsTransformation(
	tp *typeparser.TypeParser,
	c *checker.Checker,
	sf *ast.SourceFile,
	transformation *typeparser.PipingFlowTransformation,
) (CatchTagToCatchReasonMatch, bool) {
	if transformation == nil || len(transformation.Args) != 1 {
		return CatchTagToCatchReasonMatch{}, false
	}

	casesNode := unwrapTransparentExpression(transformation.Args[0])
	if casesNode == nil || casesNode.Kind != ast.KindObjectLiteralExpression {
		return CatchTagToCatchReasonMatch{}, false
	}
	cases := casesNode.AsObjectLiteralExpression()
	if cases == nil || cases.Properties == nil {
		return CatchTagToCatchReasonMatch{}, false
	}

	var candidate *catchTagToCatchReasonHandler
	for _, propertyNode := range cases.Properties.Nodes {
		if propertyNode == nil || propertyNode.Kind != ast.KindPropertyAssignment {
			continue
		}
		property := propertyNode.AsPropertyAssignment()
		if property == nil || property.Name() == nil || property.Initializer == nil {
			continue
		}
		if _, ok := catchTagsPropertyName(property.Name()); !ok {
			continue
		}

		handler, ok := analyzeCatchTagToCatchReasonHandler(tp, c, property.Initializer)
		if !ok || !hasCatchReasonApi(tp, c, transformation.Callee, len(handler.branches)) {
			continue
		}
		candidate = &handler
		if handler.swallowsUnrelatedReasons {
			break
		}
	}
	if candidate == nil {
		return CatchTagToCatchReasonMatch{}, false
	}

	return CatchTagToCatchReasonMatch{
		SourceFile:               sf,
		Location:                 scanner.GetErrorRangeForNode(sf, transformation.Node),
		CallNode:                 transformation.Node,
		Callee:                   transformation.Callee,
		ParameterName:            candidate.parameterName,
		CatchMethodName:          "catchTags",
		Branches:                 candidate.branches,
		SwallowsUnrelatedReasons: candidate.swallowsUnrelatedReasons,
		CanFix:                   false,
	}, true
}

func analyzeCatchTagToCatchReasonHandler(tp *typeparser.TypeParser, c *checker.Checker, handlerNode *ast.Node) (catchTagToCatchReasonHandler, bool) {
	handlerNode = unwrapTransparentExpression(handlerNode)
	if handlerNode == nil || (handlerNode.Kind != ast.KindArrowFunction && handlerNode.Kind != ast.KindFunctionExpression) {
		return catchTagToCatchReasonHandler{}, false
	}

	parameters := typeparser.GetFunctionLikeParameters(handlerNode)
	body := typeparser.GetFunctionLikeBody(handlerNode)
	if parameters == nil || len(parameters.Nodes) != 1 || body == nil || body.Kind != ast.KindBlock {
		return catchTagToCatchReasonHandler{}, false
	}
	parameter := parameters.Nodes[0]
	if parameter == nil || parameter.Name() == nil || parameter.Name().Kind != ast.KindIdentifier {
		return catchTagToCatchReasonHandler{}, false
	}
	parameterSymbol := tp.GetSymbolAtLocation(parameter.Name())
	if parameterSymbol == nil {
		return catchTagToCatchReasonHandler{}, false
	}
	reasonTags, ok := catchReasonLiteralTags(tp, c, parameter.Name())
	if !ok {
		return catchTagToCatchReasonHandler{}, false
	}

	controlFlow, ok := parseCatchTagToCatchReasonControlFlow(tp, c, body, parameterSymbol, reasonTags)
	if !ok {
		return catchTagToCatchReasonHandler{}, false
	}

	swallows, fallbackParam, ok := classifyCatchTagFallback(tp, c, controlFlow.fallback, parameterSymbol)
	if !ok {
		return catchTagToCatchReasonHandler{}, false
	}
	controlFlow.fallbackParam = fallbackParam

	usesReason, validUses := validateCatchTagParameterUses(tp, c, body, parameterSymbol, controlFlow)
	if !validUses {
		return catchTagToCatchReasonHandler{}, false
	}

	return catchTagToCatchReasonHandler{
		parameterName:            parameter.Name().AsIdentifier().Text,
		branches:                 controlFlow.branches,
		swallowsUnrelatedReasons: swallows,
		canFix:                   handlerNode.Kind == ast.KindArrowFunction && !swallows && !usesReason,
	}, true
}

func catchReasonLiteralTags(tp *typeparser.TypeParser, c *checker.Checker, parameterName *ast.Node) (map[string]struct{}, bool) {
	parameterType := tp.GetTypeAtLocation(parameterName)
	if parameterType == nil {
		return nil, false
	}
	reasonType := c.GetTypeOfPropertyOfType(parameterType, "reason")
	if reasonType == nil {
		reasonType = tp.GetTypeOfPropertyByName(parameterType, "reason")
	}
	if reasonType == nil {
		return nil, false
	}
	if reasonType.Flags()&checker.TypeFlagsUnion == 0 {
		return nil, false
	}

	tags := make(map[string]struct{})
	for _, member := range tp.UnrollUnionMembers(reasonType) {
		if member == nil {
			return nil, false
		}
		tagType := c.GetTypeOfPropertyOfType(member, "_tag")
		if tagType == nil {
			tagType = tp.GetTypeOfPropertyByName(member, "_tag")
		}
		if tagType == nil || tagType.Flags()&checker.TypeFlagsStringLiteral == 0 {
			return nil, false
		}
		tag, ok := tagType.AsLiteralType().Value().(string)
		if !ok {
			return nil, false
		}
		tags[tag] = struct{}{}
	}
	return tags, len(tags) > 0
}

func parseCatchTagToCatchReasonControlFlow(
	tp *typeparser.TypeParser,
	c *checker.Checker,
	body *ast.Node,
	parameterSymbol *ast.Symbol,
	reasonTags map[string]struct{},
) (catchTagToCatchReasonControlFlow, bool) {
	block := body.AsBlock()
	if block == nil || block.Statements == nil || len(block.Statements.Nodes) == 0 {
		return catchTagToCatchReasonControlFlow{}, false
	}
	statements := block.Statements.Nodes
	if len(statements) == 1 && statements[0] != nil {
		switch statements[0].Kind {
		case ast.KindSwitchStatement:
			return parseCatchReasonSwitch(tp, c, statements[0], parameterSymbol, reasonTags)
		case ast.KindIfStatement:
			return parseCatchReasonIfElse(tp, c, statements[0], parameterSymbol, reasonTags)
		}
	}
	return parseCatchReasonSequentialIfs(tp, c, statements, parameterSymbol, reasonTags)
}

func parseCatchReasonSwitch(
	tp *typeparser.TypeParser,
	c *checker.Checker,
	node *ast.Node,
	parameterSymbol *ast.Symbol,
	reasonTags map[string]struct{},
) (catchTagToCatchReasonControlFlow, bool) {
	statement := node.AsSwitchStatement()
	if statement == nil || statement.Expression == nil || statement.CaseBlock == nil {
		return catchTagToCatchReasonControlFlow{}, false
	}
	dispatchRef, ok := catchReasonTagReference(tp, c, statement.Expression, parameterSymbol)
	if !ok {
		return catchTagToCatchReasonControlFlow{}, false
	}
	if statement.CaseBlock.Kind != ast.KindCaseBlock {
		return catchTagToCatchReasonControlFlow{}, false
	}
	caseBlock := statement.CaseBlock.AsCaseBlock()
	if caseBlock == nil || caseBlock.Clauses == nil || len(caseBlock.Clauses.Nodes) < 2 {
		return catchTagToCatchReasonControlFlow{}, false
	}

	flow := catchTagToCatchReasonControlFlow{dispatchRefs: map[*ast.Node]struct{}{dispatchRef: {}}}
	seenTags := make(map[string]struct{})
	for i, clauseNode := range caseBlock.Clauses.Nodes {
		if clauseNode == nil || (clauseNode.Kind != ast.KindCaseClause && clauseNode.Kind != ast.KindDefaultClause) {
			return catchTagToCatchReasonControlFlow{}, false
		}
		clause := clauseNode.AsCaseOrDefaultClause()
		if clause == nil || clause.Statements == nil {
			return catchTagToCatchReasonControlFlow{}, false
		}
		returned := singleCatchReasonReturnExpression(clause.Statements.Nodes)
		if returned == nil {
			return catchTagToCatchReasonControlFlow{}, false
		}

		if clauseNode.Kind == ast.KindDefaultClause {
			if i != len(caseBlock.Clauses.Nodes)-1 {
				return catchTagToCatchReasonControlFlow{}, false
			}
			flow.fallback = returned
			continue
		}
		if clauseNode.Kind != ast.KindCaseClause || clause.Expression == nil || flow.fallback != nil {
			return catchTagToCatchReasonControlFlow{}, false
		}
		tagNode := unwrapTransparentExpression(clause.Expression)
		if tagNode == nil || !ast.IsStringLiteral(tagNode) {
			return catchTagToCatchReasonControlFlow{}, false
		}
		tag := tagNode.AsStringLiteral().Text
		if _, exists := reasonTags[tag]; !exists {
			return catchTagToCatchReasonControlFlow{}, false
		}
		if _, duplicate := seenTags[tag]; duplicate || !isEffectExpression(tp, returned) {
			return catchTagToCatchReasonControlFlow{}, false
		}
		seenTags[tag] = struct{}{}
		flow.branches = append(flow.branches, CatchTagToCatchReasonBranch{ReasonTag: tag, ReturnExpression: returned})
	}

	if len(flow.branches) == 0 || flow.fallback == nil {
		return catchTagToCatchReasonControlFlow{}, false
	}
	return flow, true
}

func parseCatchReasonIfElse(
	tp *typeparser.TypeParser,
	c *checker.Checker,
	node *ast.Node,
	parameterSymbol *ast.Symbol,
	reasonTags map[string]struct{},
) (catchTagToCatchReasonControlFlow, bool) {
	flow := catchTagToCatchReasonControlFlow{dispatchRefs: make(map[*ast.Node]struct{})}
	seenTags := make(map[string]struct{})
	current := node
	for current != nil && current.Kind == ast.KindIfStatement {
		statement := current.AsIfStatement()
		if statement == nil || statement.Expression == nil || statement.ThenStatement == nil {
			return catchTagToCatchReasonControlFlow{}, false
		}
		tag, dispatchRef, ok := catchReasonIfCondition(tp, c, statement.Expression, parameterSymbol, reasonTags)
		if !ok {
			return catchTagToCatchReasonControlFlow{}, false
		}
		returned := singleCatchReasonEmbeddedReturn(statement.ThenStatement)
		if returned == nil || !isEffectExpression(tp, returned) {
			return catchTagToCatchReasonControlFlow{}, false
		}
		if _, duplicate := seenTags[tag]; duplicate {
			return catchTagToCatchReasonControlFlow{}, false
		}
		seenTags[tag] = struct{}{}
		flow.dispatchRefs[dispatchRef] = struct{}{}
		flow.branches = append(flow.branches, CatchTagToCatchReasonBranch{ReasonTag: tag, ReturnExpression: returned})

		if statement.ElseStatement == nil {
			return catchTagToCatchReasonControlFlow{}, false
		}
		if statement.ElseStatement.Kind == ast.KindIfStatement {
			current = statement.ElseStatement
			continue
		}
		flow.fallback = singleCatchReasonEmbeddedReturn(statement.ElseStatement)
		current = nil
	}
	if len(flow.branches) == 0 || flow.fallback == nil {
		return catchTagToCatchReasonControlFlow{}, false
	}
	return flow, true
}

func parseCatchReasonSequentialIfs(
	tp *typeparser.TypeParser,
	c *checker.Checker,
	statements []*ast.Node,
	parameterSymbol *ast.Symbol,
	reasonTags map[string]struct{},
) (catchTagToCatchReasonControlFlow, bool) {
	if len(statements) < 2 {
		return catchTagToCatchReasonControlFlow{}, false
	}
	fallback := singleCatchReasonEmbeddedReturn(statements[len(statements)-1])
	if fallback == nil {
		return catchTagToCatchReasonControlFlow{}, false
	}

	flow := catchTagToCatchReasonControlFlow{fallback: fallback, dispatchRefs: make(map[*ast.Node]struct{})}
	seenTags := make(map[string]struct{})
	for _, node := range statements[:len(statements)-1] {
		if node == nil || node.Kind != ast.KindIfStatement {
			return catchTagToCatchReasonControlFlow{}, false
		}
		statement := node.AsIfStatement()
		if statement == nil || statement.Expression == nil || statement.ThenStatement == nil || statement.ElseStatement != nil {
			return catchTagToCatchReasonControlFlow{}, false
		}
		tag, dispatchRef, ok := catchReasonIfCondition(tp, c, statement.Expression, parameterSymbol, reasonTags)
		if !ok {
			return catchTagToCatchReasonControlFlow{}, false
		}
		returned := singleCatchReasonEmbeddedReturn(statement.ThenStatement)
		if returned == nil || !isEffectExpression(tp, returned) {
			return catchTagToCatchReasonControlFlow{}, false
		}
		if _, duplicate := seenTags[tag]; duplicate {
			return catchTagToCatchReasonControlFlow{}, false
		}
		seenTags[tag] = struct{}{}
		flow.dispatchRefs[dispatchRef] = struct{}{}
		flow.branches = append(flow.branches, CatchTagToCatchReasonBranch{ReasonTag: tag, ReturnExpression: returned})
	}
	return flow, len(flow.branches) > 0
}

func catchReasonIfCondition(
	tp *typeparser.TypeParser,
	c *checker.Checker,
	expression *ast.Node,
	parameterSymbol *ast.Symbol,
	reasonTags map[string]struct{},
) (string, *ast.Node, bool) {
	expression = unwrapTransparentExpression(expression)
	if expression == nil || expression.Kind != ast.KindBinaryExpression {
		return "", nil, false
	}
	binary := expression.AsBinaryExpression()
	if binary == nil || binary.Left == nil || binary.Right == nil || binary.OperatorToken == nil ||
		(binary.OperatorToken.Kind != ast.KindEqualsEqualsToken && binary.OperatorToken.Kind != ast.KindEqualsEqualsEqualsToken) {
		return "", nil, false
	}

	left := unwrapTransparentExpression(binary.Left)
	right := unwrapTransparentExpression(binary.Right)
	var tagNode *ast.Node
	var dispatchRef *ast.Node
	var ok bool
	if left != nil && ast.IsStringLiteral(left) {
		tagNode = left
		dispatchRef, ok = catchReasonTagReference(tp, c, right, parameterSymbol)
	} else if right != nil && ast.IsStringLiteral(right) {
		tagNode = right
		dispatchRef, ok = catchReasonTagReference(tp, c, left, parameterSymbol)
	}
	if !ok || tagNode == nil {
		return "", nil, false
	}
	tag := tagNode.AsStringLiteral().Text
	_, exists := reasonTags[tag]
	return tag, dispatchRef, exists
}

func catchReasonTagReference(tp *typeparser.TypeParser, c *checker.Checker, node *ast.Node, parameterSymbol *ast.Symbol) (*ast.Node, bool) {
	node = unwrapTransparentExpression(node)
	if node == nil || node.Kind != ast.KindPropertyAccessExpression {
		return nil, false
	}
	tagAccess := node.AsPropertyAccessExpression()
	if tagAccess == nil || tagAccess.Expression == nil || tagAccess.Name() == nil || tagAccess.Name().Text() != "_tag" {
		return nil, false
	}
	reasonNode := unwrapTransparentExpression(tagAccess.Expression)
	if reasonNode == nil || reasonNode.Kind != ast.KindPropertyAccessExpression {
		return nil, false
	}
	reasonAccess := reasonNode.AsPropertyAccessExpression()
	if reasonAccess == nil || reasonAccess.Expression == nil || reasonAccess.Name() == nil || reasonAccess.Name().Text() != "reason" {
		return nil, false
	}
	root := unwrapTransparentExpression(reasonAccess.Expression)
	if root == nil || root.Kind != ast.KindIdentifier || !sameCatchReasonSymbol(tp, c, root, parameterSymbol) {
		return nil, false
	}
	return root, true
}

func classifyCatchTagFallback(tp *typeparser.TypeParser, c *checker.Checker, expression *ast.Node, parameterSymbol *ast.Symbol) (bool, *ast.Node, bool) {
	expression = unwrapTransparentExpression(expression)
	if expression == nil {
		return false, nil, false
	}
	if expression.Kind == ast.KindCallExpression {
		call := expression.AsCallExpression()
		if call != nil && call.Expression != nil && call.Arguments != nil && len(call.Arguments.Nodes) == 1 &&
			tp.IsNodeReferenceToEffectModuleApi(call.Expression, "fail") {
			argument := unwrapTransparentExpression(call.Arguments.Nodes[0])
			if argument != nil && argument.Kind == ast.KindIdentifier && sameCatchReasonSymbol(tp, c, argument, parameterSymbol) {
				return false, argument, true
			}
		}
	}

	if expression.Kind == ast.KindCallExpression {
		call := expression.AsCallExpression()
		if call != nil && call.Expression != nil && tp.IsNodeReferenceToEffectModuleApi(call.Expression, "succeed") {
			return true, nil, true
		}
	}
	if tp.IsNodeReferenceToEffectModuleApi(expression, "void") {
		return true, nil, true
	}
	return false, nil, false
}

func validateCatchTagParameterUses(
	tp *typeparser.TypeParser,
	c *checker.Checker,
	body *ast.Node,
	parameterSymbol *ast.Symbol,
	flow catchTagToCatchReasonControlFlow,
) (bool, bool) {
	allowed := make(map[*ast.Node]struct{}, len(flow.dispatchRefs)+1)
	for node := range flow.dispatchRefs {
		allowed[node] = struct{}{}
	}
	if flow.fallbackParam != nil {
		allowed[flow.fallbackParam] = struct{}{}
	}

	usesReason := false
	valid := true
	var walkBody ast.Visitor
	walkBody = func(node *ast.Node) bool {
		if node == nil || !valid {
			return true
		}
		if node.Kind == ast.KindIdentifier && sameCatchReasonSymbol(tp, c, node, parameterSymbol) {
			if _, ok := allowed[node]; ok {
				return false
			}
			if isCatchReasonRootReference(node) && isCatchReasonRecoveryReference(node, flow.branches) {
				usesReason = true
				return false
			}
			valid = false
			return true
		}
		node.ForEachChild(walkBody)
		return false
	}
	walkBody(body)
	return usesReason, valid
}

func isCatchReasonRecoveryReference(node *ast.Node, branches []CatchTagToCatchReasonBranch) bool {
	for _, branch := range branches {
		expression := branch.ReturnExpression
		if expression != nil && node.Pos() >= expression.Pos() && node.End() <= expression.End() {
			return true
		}
	}
	return false
}

func isCatchReasonRootReference(node *ast.Node) bool {
	if node == nil || node.Parent == nil || node.Parent.Kind != ast.KindPropertyAccessExpression {
		return false
	}
	access := node.Parent.AsPropertyAccessExpression()
	return access != nil && access.Expression == node && access.Name() != nil && access.Name().Text() == "reason"
}

func singleCatchReasonEmbeddedReturn(statement *ast.Node) *ast.Node {
	if statement == nil {
		return nil
	}
	if statement.Kind == ast.KindReturnStatement {
		returned := statement.AsReturnStatement()
		if returned != nil {
			return returned.Expression
		}
		return nil
	}
	if statement.Kind != ast.KindBlock {
		return nil
	}
	block := statement.AsBlock()
	if block == nil || block.Statements == nil {
		return nil
	}
	return singleCatchReasonReturnExpression(block.Statements.Nodes)
}

func singleCatchReasonReturnExpression(statements []*ast.Node) *ast.Node {
	if len(statements) != 1 || statements[0] == nil || statements[0].Kind != ast.KindReturnStatement {
		return nil
	}
	returned := statements[0].AsReturnStatement()
	if returned == nil {
		return nil
	}
	return returned.Expression
}

func isEffectExpression(tp *typeparser.TypeParser, expression *ast.Node) bool {
	return expression != nil && tp.EffectType(tp.GetTypeAtLocation(expression), expression) != nil
}

func hasCatchReasonApi(tp *typeparser.TypeParser, c *checker.Checker, callee *ast.Node, branchCount int) bool {
	callee = unwrapTransparentExpression(callee)
	if callee == nil || callee.Kind != ast.KindPropertyAccessExpression {
		return false
	}
	access := callee.AsPropertyAccessExpression()
	if access == nil || access.Expression == nil {
		return false
	}
	receiverType := tp.GetTypeAtLocation(access.Expression)
	if receiverType == nil {
		return false
	}
	apiName := "catchReason"
	if branchCount > 1 {
		apiName = "catchReasons"
	}
	return c.GetPropertyOfType(receiverType, apiName) != nil
}

func transformationCallHasExactArgs(transformation *typeparser.PipingFlowTransformation) bool {
	if transformation == nil || transformation.Node == nil || transformation.Node.Kind != ast.KindCallExpression {
		return false
	}
	call := transformation.Node.AsCallExpression()
	if call == nil || call.Arguments == nil || len(call.Arguments.Nodes) != len(transformation.Args) {
		return false
	}
	for i, argument := range call.Arguments.Nodes {
		if argument != transformation.Args[i] {
			return false
		}
	}
	return true
}

func catchTagsPropertyName(name *ast.Node) (string, bool) {
	if name == nil {
		return "", false
	}
	switch name.Kind {
	case ast.KindIdentifier:
		return name.AsIdentifier().Text, true
	case ast.KindStringLiteral:
		return name.AsStringLiteral().Text, true
	default:
		return "", false
	}
}

func sameCatchReasonSymbol(tp *typeparser.TypeParser, c *checker.Checker, node *ast.Node, expected *ast.Symbol) bool {
	actual := tp.GetSymbolAtLocation(node)
	return actual != nil && expected != nil && checker.Checker_getSymbolIfSameReference(c, actual, expected) != nil
}
