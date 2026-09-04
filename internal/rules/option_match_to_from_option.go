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

// OptionMatchToFromOption suggests Effect.fromOption when Option.match or an
// Option tag conditional only lifts Some with Effect.succeed and None with
// Effect.fail.
var OptionMatchToFromOption = rule.Rule{
	Name:            "optionMatchToFromOption",
	Group:           "style",
	Description:     "Suggests Effect.fromOption when Option.match or an Option tag conditional only converts Some to Effect.succeed and None to Effect.fail",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v4"},
	Codes: []int32{
		tsdiag.Effect_fromOption_expresses_this_Option_to_Effect_conversion_more_directly_than_Option_match_or_an_Option_tag_conditional_effect_optionMatchToFromOption.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeOptionMatchToFromOption(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diagnostics := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diagnostics[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.Effect_fromOption_expresses_this_Option_to_Effect_conversion_more_directly_than_Option_match_or_an_Option_tag_conditional_effect_optionMatchToFromOption,
				nil,
			)
		}
		return diagnostics
	},
}

// OptionMatchToFromOptionMatch holds the nodes needed by the diagnostic and
// quick fix.
type OptionMatchToFromOptionMatch struct {
	SourceFile       *ast.SourceFile
	Location         core.TextRange
	Transformation   *typeparser.PipingFlowTransformation
	ReplacementNode  *ast.Node
	EffectModuleNode *ast.Node
	OptionNode       *ast.Node
	FailureNode      *ast.Node
	DefaultFailure   bool
	CanFix           bool
}

// AnalyzeOptionMatchToFromOption finds Option.match and Option result
// dispatches that are equivalent to Effect.fromOption.
func AnalyzeOptionMatchToFromOption(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []OptionMatchToFromOptionMatch {
	if tp == nil || c == nil || sf == nil || tp.SupportedEffectVersion() != typeparser.EffectMajorV4 {
		return nil
	}

	matches := analyzeOptionMatchCalls(tp, sf)
	matches = append(matches, analyzeOptionConditionals(tp, c, sf)...)
	return matches
}

func analyzeOptionMatchCalls(tp *typeparser.TypeParser, sf *ast.SourceFile) []OptionMatchToFromOptionMatch {
	var matches []OptionMatchToFromOptionMatch

	for _, flow := range tp.PipingFlows(sf, true) {
		for index := range flow.Transformations {
			transformation := &flow.Transformations[index]
			callee := transformation.Callee
			args := transformation.Args
			if len(args) != 1 || !tp.IsNodeReferenceToEffectOptionModuleApi(callee, "match") {
				continue
			}

			failure, effectModule, canFix, ok := analyzeOptionMatchHandlers(tp, args[0])
			if !ok {
				continue
			}

			match := OptionMatchToFromOptionMatch{
				SourceFile:       sf,
				Location:         scanner.GetErrorRangeForNode(sf, callee),
				Transformation:   transformation,
				EffectModuleNode: effectModule,
				FailureNode:      failure,
				DefaultFailure:   isDefaultNoSuchElementError(tp, failure),
				CanFix:           canFix && effectModule != nil,
			}

			switch transformation.Kind {
			case typeparser.TransformationKindPipe,
				typeparser.TransformationKindPipeable,
				typeparser.TransformationKindDataFirst,
				typeparser.TransformationKindDataLast,
				typeparser.TransformationKindCall:
			default:
				continue
			}

			matches = append(matches, match)
		}
	}

	return matches
}

func analyzeOptionMatchHandlers(tp *typeparser.TypeParser, node *ast.Node) (failure *ast.Node, effectModule *ast.Node, canFix bool, ok bool) {
	node = ast.SkipParentheses(node)
	if node == nil || node.Kind != ast.KindObjectLiteralExpression {
		return nil, nil, false, false
	}
	object := node.AsObjectLiteralExpression()
	if object == nil || object.Properties == nil || len(object.Properties.Nodes) != 2 {
		return nil, nil, false, false
	}

	var onNone, onSome *ast.Node
	for _, propertyNode := range object.Properties.Nodes {
		if propertyNode == nil || propertyNode.Kind != ast.KindPropertyAssignment {
			return nil, nil, false, false
		}
		property := propertyNode.AsPropertyAssignment()
		if property == nil || property.Name() == nil || property.Name().Kind != ast.KindIdentifier || property.Initializer == nil {
			return nil, nil, false, false
		}
		switch scanner.GetTextOfNode(property.Name()) {
		case "onNone":
			if onNone != nil {
				return nil, nil, false, false
			}
			onNone = property.Initializer
		case "onSome":
			if onSome != nil {
				return nil, nil, false, false
			}
			onSome = property.Initializer
		default:
			return nil, nil, false, false
		}
	}

	succeedModule, ok := analyzeEffectSucceedHandler(tp, onSome)
	if !ok {
		return nil, nil, false, false
	}
	failure, failModule, canFix, ok := analyzeEffectFailHandler(tp, onNone)
	if !ok {
		return nil, nil, false, false
	}
	if succeedModule != nil {
		return failure, succeedModule, canFix, true
	}
	return failure, failModule, canFix, true
}

func analyzeEffectSucceedHandler(tp *typeparser.TypeParser, node *ast.Node) (*ast.Node, bool) {
	target, typeArguments, _ := tp.UnwrapIdentityForwarder(node)
	if typeArguments != nil && len(typeArguments.Nodes) > 0 || !tp.IsNodeReferenceToEffectModuleApi(target, "succeed") {
		return nil, false
	}
	return effectModuleExpression(target), true
}

func analyzeEffectFailHandler(tp *typeparser.TypeParser, node *ast.Node) (*ast.Node, *ast.Node, bool, bool) {
	lazy := typeparser.ParseLazyExpression(ast.SkipParentheses(node), true)
	if lazy == nil || !isSynchronousFunction(lazy.Node) {
		return nil, nil, false, false
	}
	expression := ast.SkipParentheses(lazy.Expression)
	if expression == nil || expression.Kind != ast.KindCallExpression {
		return nil, nil, false, false
	}
	call := expression.AsCallExpression()
	if call == nil || call.Expression == nil || call.Arguments == nil || len(call.Arguments.Nodes) != 1 ||
		call.TypeArguments != nil && len(call.TypeArguments.Nodes) > 0 ||
		!tp.IsNodeReferenceToEffectModuleApi(call.Expression, "fail") {
		return nil, nil, false, false
	}
	return call.Arguments.Nodes[0], effectModuleExpression(call.Expression), lazy.Node.Kind == ast.KindArrowFunction, true
}

func isSynchronousFunction(node *ast.Node) bool {
	if node == nil || ast.GetCombinedModifierFlags(node)&ast.ModifierFlagsAsync != 0 {
		return false
	}
	if node.Kind == ast.KindFunctionExpression {
		function := node.AsFunctionExpression()
		return function != nil && function.AsteriskToken == nil
	}
	return node.Kind == ast.KindArrowFunction
}

func effectModuleExpression(node *ast.Node) *ast.Node {
	node = ast.SkipParentheses(node)
	if node == nil || node.Kind != ast.KindPropertyAccessExpression {
		return nil
	}
	property := node.AsPropertyAccessExpression()
	if property == nil || property.QuestionDotToken != nil {
		return nil
	}
	receiver := ast.SkipParentheses(property.Expression)
	if receiver == nil || receiver.Kind != ast.KindIdentifier {
		return nil
	}
	return receiver
}

func analyzeOptionConditionals(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []OptionMatchToFromOptionMatch {
	var matches []OptionMatchToFromOptionMatch
	var visit func(*ast.Node) bool
	visit = func(node *ast.Node) bool {
		if node == nil {
			return false
		}
		if node.Kind == ast.KindConditionalExpression {
			if match, ok := analyzeOptionConditional(tp, c, sf, node); ok {
				matches = append(matches, match)
			}
		}
		node.ForEachChild(visit)
		return false
	}
	sf.AsNode().ForEachChild(visit)
	return matches
}

func analyzeOptionConditional(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile, node *ast.Node) (OptionMatchToFromOptionMatch, bool) {
	dispatch := typeparser.ParseResultDispatch(node)
	if dispatch == nil || len(dispatch.Branches) != 1 || dispatch.Fallback == nil {
		return OptionMatchToFromOptionMatch{}, false
	}
	branch := dispatch.Branches[0]
	if branch.Condition.Kind != typeparser.DispatchConditionPredicate || branch.Condition.Subject == nil || branch.Condition.Value != nil {
		return OptionMatchToFromOptionMatch{}, false
	}

	conditionNode := ast.SkipParentheses(branch.Condition.Subject)
	if conditionNode == nil || conditionNode.Kind != ast.KindCallExpression {
		return OptionMatchToFromOptionMatch{}, false
	}
	condition := conditionNode.AsCallExpression()
	if condition == nil || condition.Expression == nil || condition.Arguments == nil || len(condition.Arguments.Nodes) != 1 ||
		condition.TypeArguments != nil && len(condition.TypeArguments.Nodes) > 0 {
		return OptionMatchToFromOptionMatch{}, false
	}

	isSome := tp.IsNodeReferenceToEffectOptionModuleApi(condition.Expression, "isSome")
	isNone := tp.IsNodeReferenceToEffectOptionModuleApi(condition.Expression, "isNone")
	if isSome == isNone {
		return OptionMatchToFromOptionMatch{}, false
	}
	optionNode := ast.SkipParentheses(condition.Arguments.Nodes[0])
	if optionNode == nil || optionNode.Kind != ast.KindIdentifier {
		return OptionMatchToFromOptionMatch{}, false
	}

	succeedNode := branch.Result
	failNode := dispatch.Fallback
	if isNone {
		succeedNode, failNode = failNode, succeedNode
	}

	effectModule, ok := analyzeConditionalSucceed(tp, c, optionNode, succeedNode)
	if !ok {
		return OptionMatchToFromOptionMatch{}, false
	}
	failure, failModule, ok := analyzeEffectFailCall(tp, failNode)
	if !ok {
		return OptionMatchToFromOptionMatch{}, false
	}
	if effectModule == nil {
		effectModule = failModule
	}

	return OptionMatchToFromOptionMatch{
		SourceFile:       sf,
		Location:         scanner.GetErrorRangeForNode(sf, condition.Expression),
		ReplacementNode:  dispatch.Node,
		EffectModuleNode: effectModule,
		OptionNode:       optionNode,
		FailureNode:      failure,
		DefaultFailure:   isDefaultNoSuchElementError(tp, failure),
		CanFix:           effectModule != nil,
	}, true
}

func analyzeConditionalSucceed(tp *typeparser.TypeParser, c *checker.Checker, optionNode *ast.Node, node *ast.Node) (*ast.Node, bool) {
	expression := ast.SkipParentheses(node)
	if expression == nil || expression.Kind != ast.KindCallExpression {
		return nil, false
	}
	call := expression.AsCallExpression()
	if call == nil || call.Expression == nil || call.Arguments == nil || len(call.Arguments.Nodes) != 1 ||
		call.TypeArguments != nil && len(call.TypeArguments.Nodes) > 0 ||
		!tp.IsNodeReferenceToEffectModuleApi(call.Expression, "succeed") {
		return nil, false
	}

	valueNode := ast.SkipParentheses(call.Arguments.Nodes[0])
	if valueNode == nil || valueNode.Kind != ast.KindPropertyAccessExpression {
		return nil, false
	}
	value := valueNode.AsPropertyAccessExpression()
	if value == nil || value.QuestionDotToken != nil || value.Name() == nil || scanner.GetTextOfNode(value.Name()) != "value" ||
		!sameIdentifierReference(tp, c, optionNode, ast.SkipParentheses(value.Expression)) {
		return nil, false
	}
	return effectModuleExpression(call.Expression), true
}

func analyzeEffectFailCall(tp *typeparser.TypeParser, node *ast.Node) (*ast.Node, *ast.Node, bool) {
	expression := ast.SkipParentheses(node)
	if expression == nil || expression.Kind != ast.KindCallExpression {
		return nil, nil, false
	}
	call := expression.AsCallExpression()
	if call == nil || call.Expression == nil || call.Arguments == nil || len(call.Arguments.Nodes) != 1 ||
		call.TypeArguments != nil && len(call.TypeArguments.Nodes) > 0 ||
		!tp.IsNodeReferenceToEffectModuleApi(call.Expression, "fail") {
		return nil, nil, false
	}
	return call.Arguments.Nodes[0], effectModuleExpression(call.Expression), true
}

func sameIdentifierReference(tp *typeparser.TypeParser, c *checker.Checker, left *ast.Node, right *ast.Node) bool {
	if left == nil || right == nil || left.Kind != ast.KindIdentifier || right.Kind != ast.KindIdentifier {
		return false
	}
	leftSymbol := tp.GetSymbolAtLocation(left)
	rightSymbol := tp.GetSymbolAtLocation(right)
	return leftSymbol != nil && rightSymbol != nil && checker.Checker_getSymbolIfSameReference(c, leftSymbol, rightSymbol) != nil
}

func isDefaultNoSuchElementError(tp *typeparser.TypeParser, node *ast.Node) bool {
	node = ast.SkipParentheses(node)
	if node == nil || node.Kind != ast.KindNewExpression {
		return false
	}
	expression := node.AsNewExpression()
	if expression == nil || expression.Expression == nil ||
		expression.TypeArguments != nil && len(expression.TypeArguments.Nodes) > 0 ||
		expression.Arguments != nil && len(expression.Arguments.Nodes) > 0 {
		return false
	}
	return tp.IsNodeReferenceToEffectCauseModuleApi(expression.Expression, "NoSuchElementError")
}
