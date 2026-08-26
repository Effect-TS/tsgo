package rules

import (
	"strconv"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

var CatchTagToCatchReason = rule.Rule{
	Name:            "catchTagToCatchReason",
	Group:           "style",
	Description:     "Suggests Effect.catchReason or Effect.catchReasons for handlers that re-fail unmatched reason._tag branches",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v4"},
	Codes: []int32{
		tsdiag.Branching_on_0_reason_tag_inside_Effect_1_hand_rolls_reason_dispatch_use_Effect_catchReason_or_Effect_catchReasons_which_re_fail_unmatched_reasons_automatically_effect_catchTagToCatchReason.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeCatchTagToCatchReason(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diagnostics := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diagnostics[i] = ctx.NewDiagnostic(match.SourceFile, match.Location, tsdiag.Branching_on_0_reason_tag_inside_Effect_1_hand_rolls_reason_dispatch_use_Effect_catchReason_or_Effect_catchReasons_which_re_fail_unmatched_reasons_automatically_effect_catchTagToCatchReason, nil, match.ParameterName, match.CatchMethodName)
		}
		return diagnostics
	},
}

type CatchTagToCatchReasonBranch struct {
	ReasonTag           string
	Result              *ast.Node
	UsesParameter       bool
	ReasonParameterName string
}

type CatchTagToCatchReasonMatch struct {
	SourceFile      *ast.SourceFile
	Location        core.TextRange
	CallNode        *ast.Node
	Callee          *ast.Node
	OuterTag        *ast.Node
	ParameterName   string
	CatchMethodName string
	Branches        []CatchTagToCatchReasonBranch
	CanFix          bool
}

type catchTagToCatchReasonHandler struct {
	parameterName string
	branches      []CatchTagToCatchReasonBranch
	canFix        bool
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
		SourceFile:      sf,
		Location:        scanner.GetErrorRangeForNode(sf, transformation.Node),
		CallNode:        transformation.Node,
		Callee:          transformation.Callee,
		OuterTag:        outerTag,
		ParameterName:   handler.parameterName,
		CatchMethodName: "catchTag",
		Branches:        handler.branches,
		CanFix:          canFix,
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
		break
	}
	if candidate == nil {
		return CatchTagToCatchReasonMatch{}, false
	}

	return CatchTagToCatchReasonMatch{
		SourceFile:      sf,
		Location:        scanner.GetErrorRangeForNode(sf, transformation.Node),
		CallNode:        transformation.Node,
		Callee:          transformation.Callee,
		ParameterName:   candidate.parameterName,
		CatchMethodName: "catchTags",
		Branches:        candidate.branches,
		CanFix:          false,
	}, true
}

func analyzeCatchTagToCatchReasonHandler(tp *typeparser.TypeParser, c *checker.Checker, handlerNode *ast.Node) (catchTagToCatchReasonHandler, bool) {
	returning := typeparser.ParseReturningDispatch(handlerNode)
	if returning == nil || len(returning.Params) != 1 {
		return catchTagToCatchReasonHandler{}, false
	}
	handlerNode = returning.Node
	body := returning.Body
	parameter := returning.Params[0]
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

	dispatchRefs := make(map[*ast.Node]struct{})
	dispatch := tp.ParseTaggedDispatch(handlerNode, parameterSymbol)
	if dispatch == nil || len(dispatch.Branches) == 0 || dispatch.Fallback == nil {
		return catchTagToCatchReasonHandler{}, false
	}

	branches := make([]CatchTagToCatchReasonBranch, len(dispatch.Branches))
	for index, branch := range dispatch.Branches {
		root, exactReasonChain := catchReasonTagReference(tp, c, branch.Discriminant, parameterSymbol)
		if _, exists := reasonTags[branch.Tag]; !exactReasonChain || !exists || !isEffectExpression(tp, branch.Result) {
			return catchTagToCatchReasonHandler{}, false
		}
		dispatchRefs[root] = struct{}{}
		branches[index] = CatchTagToCatchReasonBranch{ReasonTag: branch.Tag, Result: branch.Result}
	}

	fallbackParam, ok := catchTagReFailParameter(tp, c, dispatch.Fallback, parameterSymbol)
	if !ok {
		return catchTagToCatchReasonHandler{}, false
	}

	branchParameterUses, validUses := validateCatchTagParameterUses(tp, c, body, parameterSymbol, branches, fallbackParam, dispatchRefs)
	if !validUses {
		return catchTagToCatchReasonHandler{}, false
	}
	for i := range branches {
		branches[i].UsesParameter = branchParameterUses[i]
		if branches[i].UsesParameter {
			branches[i].ReasonParameterName = uniqueCatchReasonParameterName(c, branches[i].Result)
		}
	}

	return catchTagToCatchReasonHandler{
		parameterName: parameter.Name().AsIdentifier().Text,
		branches:      branches,
		canFix:        handlerNode.Kind == ast.KindArrowFunction && !checker.Checker_isSymbolAssigned(c, parameterSymbol),
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

func catchTagReFailParameter(tp *typeparser.TypeParser, c *checker.Checker, expression *ast.Node, parameterSymbol *ast.Symbol) (*ast.Node, bool) {
	expression = unwrapTransparentExpression(expression)
	if expression == nil {
		return nil, false
	}
	if expression.Kind == ast.KindCallExpression {
		call := expression.AsCallExpression()
		if call != nil && call.Expression != nil && call.Arguments != nil && len(call.Arguments.Nodes) == 1 &&
			tp.IsNodeReferenceToEffectModuleApi(call.Expression, "fail") {
			argument := unwrapTransparentExpression(call.Arguments.Nodes[0])
			if argument != nil && argument.Kind == ast.KindIdentifier && sameCatchReasonSymbol(tp, c, argument, parameterSymbol) {
				return argument, true
			}
		}
	}
	return nil, false
}

func validateCatchTagParameterUses(
	tp *typeparser.TypeParser,
	c *checker.Checker,
	body *ast.Node,
	parameterSymbol *ast.Symbol,
	branches []CatchTagToCatchReasonBranch,
	fallbackParam *ast.Node,
	dispatchRefs map[*ast.Node]struct{},
) ([]bool, bool) {
	allowed := make(map[*ast.Node]struct{}, len(dispatchRefs)+1)
	for node := range dispatchRefs {
		allowed[node] = struct{}{}
	}
	if fallbackParam != nil {
		allowed[fallbackParam] = struct{}{}
	}

	branchUses := make([]bool, len(branches))
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
			if branchIndex := catchReasonRecoveryBranchIndex(node, branches); branchIndex >= 0 {
				branchUses[branchIndex] = true
				return false
			}
			valid = false
			return true
		}
		node.ForEachChild(walkBody)
		return false
	}
	walkBody(body)
	return branchUses, valid
}

func catchReasonRecoveryBranchIndex(node *ast.Node, branches []CatchTagToCatchReasonBranch) int {
	for i, branch := range branches {
		expression := branch.Result
		if expression != nil && node.Pos() >= expression.Pos() && node.End() <= expression.End() {
			return i
		}
	}
	return -1
}

func uniqueCatchReasonParameterName(c *checker.Checker, location *ast.Node) string {
	used := make(map[string]struct{})
	for _, symbol := range c.GetSymbolsInScope(location, ast.SymbolFlagsValue) {
		used[c.SymbolToString(symbol)] = struct{}{}
	}
	for i := -1; ; i++ {
		name := "_"
		if i >= 0 {
			name = "_" + strconv.Itoa(i)
		}
		if _, exists := used[name]; !exists {
			return name
		}
	}
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
