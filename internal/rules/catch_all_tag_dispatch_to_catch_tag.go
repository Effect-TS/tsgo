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

var CatchAllTagDispatchToCatchTag = rule.Rule{
	Name:            "catchAllTagDispatchToCatchTag",
	Group:           "style",
	Description:     "Suggests Effect.catchTag or Effect.catchTags for catch-all handlers that re-fail unmatched tagged errors",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Branching_on_0_tag_inside_Effect_1_hand_rolls_tagged_error_dispatch_use_Effect_catchTag_or_Effect_catchTags_which_re_fail_unmatched_errors_automatically_effect_catchAllTagDispatchToCatchTag.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeCatchAllTagDispatchToCatchTag(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diagnostics := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diagnostics[i] = ctx.NewDiagnostic(match.SourceFile, match.Location, tsdiag.Branching_on_0_tag_inside_Effect_1_hand_rolls_tagged_error_dispatch_use_Effect_catchTag_or_Effect_catchTags_which_re_fail_unmatched_errors_automatically_effect_catchAllTagDispatchToCatchTag, nil, match.ParameterName, match.CatchMethodName)
		}
		return diagnostics
	},
}

type CatchAllTagDispatchBranch struct {
	Tag           string
	Result        *ast.Node
	UsesParameter bool
}

type CatchAllTagDispatchMatch struct {
	SourceFile      *ast.SourceFile
	Location        core.TextRange
	Transformation  *typeparser.PipingFlowTransformation
	Callee          *ast.Node
	ParameterName   string
	CatchMethodName string
	Branches        []CatchAllTagDispatchBranch
	CanFix          bool
}

func AnalyzeCatchAllTagDispatchToCatchTag(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []CatchAllTagDispatchMatch {
	if tp == nil || c == nil || sf == nil {
		return nil
	}

	var matches []CatchAllTagDispatchMatch
	for _, flow := range tp.PipingFlows(sf, true) {
		for index := range flow.Transformations {
			transformation := &flow.Transformations[index]
			catchMethod, ok := catchAllTagDispatchMethod(tp, transformation.Callee)
			if !ok || len(transformation.Args) != 1 {
				continue
			}
			parameterName, branches, canFix, ok := analyzeCatchAllTagDispatchHandler(tp, c, transformation.Args[0])
			if !ok {
				continue
			}
			matches = append(matches, CatchAllTagDispatchMatch{
				SourceFile: sf, Location: scanner.GetErrorRangeForNode(sf, transformation.Callee),
				Transformation: transformation, Callee: transformation.Callee,
				ParameterName: parameterName, CatchMethodName: catchMethod,
				Branches: branches, CanFix: canFix,
			})
		}
	}
	return matches
}

func catchAllTagDispatchMethod(tp *typeparser.TypeParser, callee *ast.Node) (string, bool) {
	if tp.IsNodeReferenceToEffectModuleApi(callee, "catch") {
		return "catch", true
	}
	if tp.IsNodeReferenceToEffectModuleApi(callee, "catchAll") {
		return "catchAll", true
	}
	return "", false
}

func analyzeCatchAllTagDispatchHandler(tp *typeparser.TypeParser, c *checker.Checker, handlerNode *ast.Node) (string, []CatchAllTagDispatchBranch, bool, bool) {
	returning := typeparser.ParseReturningDispatch(handlerNode)
	if returning == nil || len(returning.Params) != 1 || returning.Dispatch == nil || len(returning.Dispatch.Branches) == 0 || returning.Dispatch.Fallback == nil {
		return "", nil, false, false
	}
	parameter := returning.Params[0]
	if parameter == nil || parameter.Name() == nil || parameter.Name().Kind != ast.KindIdentifier {
		return "", nil, false, false
	}
	parameterSymbol := tp.GetSymbolAtLocation(parameter.Name())
	if parameterSymbol == nil {
		return "", nil, false, false
	}
	tags, ok := literalTaggedUnionTags(tp, c, parameter.Name())
	if !ok {
		return "", nil, false, false
	}
	tagSubject := returning.Dispatch.CommonTagSubject(tp)
	if tagSubject == nil || !isBareParameterTagReference(tp, c, tagSubject, parameterSymbol) {
		return "", nil, false, false
	}

	branches := make([]CatchAllTagDispatchBranch, len(returning.Dispatch.Branches))
	seen := make(map[string]struct{}, len(branches))
	for i, branch := range returning.Dispatch.Branches {
		tag, tagged := resultDispatchTagValue(branch.Condition)
		_, validTag := tags[tag]
		_, duplicate := seen[tag]
		if !tagged || !validTag || duplicate || !isEffectExpression(tp, branch.Result) {
			return "", nil, false, false
		}
		seen[tag] = struct{}{}
		branches[i] = CatchAllTagDispatchBranch{Tag: tag, Result: branch.Result, UsesParameter: conditionalRefailNodeContainsParameter(tp, c, branch.Result, parameterSymbol)}
	}
	if _, ok := catchTagReFailParameter(tp, c, returning.Dispatch.Fallback, parameterSymbol); !ok {
		return "", nil, false, false
	}
	return parameter.Name().AsIdentifier().Text, branches, returning.Node.Kind == ast.KindArrowFunction && !checker.Checker_isSymbolAssigned(c, parameterSymbol), true
}

func literalTaggedUnionTags(tp *typeparser.TypeParser, c *checker.Checker, node *ast.Node) (map[string]struct{}, bool) {
	type_ := tp.GetTypeAtLocation(node)
	if type_ == nil || type_.Flags()&checker.TypeFlagsUnion == 0 {
		return nil, false
	}
	tags := make(map[string]struct{})
	for _, member := range tp.UnrollUnionMembers(type_) {
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

func isBareParameterTagReference(tp *typeparser.TypeParser, c *checker.Checker, node *ast.Node, parameterSymbol *ast.Symbol) bool {
	node = unwrapTransparentExpression(node)
	if node == nil || node.Kind != ast.KindIdentifier {
		return false
	}
	return sameCatchReasonSymbol(tp, c, node, parameterSymbol)
}
