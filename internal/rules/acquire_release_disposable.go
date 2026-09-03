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

// AcquireReleaseDisposable suggests Effect.acquireDisposable when an
// acquireRelease finalizer only invokes the acquired resource's JavaScript
// disposal protocol.
var AcquireReleaseDisposable = rule.Rule{
	Name:            "acquireReleaseDisposable",
	Group:           "style",
	Description:     "Suggests Effect.acquireDisposable when Effect.acquireRelease only invokes the acquired resource's disposal protocol",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v4"},
	Codes: []int32{
		tsdiag.Effect_acquireDisposable_expresses_this_disposable_resource_acquisition_more_directly_than_Effect_acquireRelease_with_a_manual_disposal_finalizer_effect_acquireReleaseDisposable.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeAcquireReleaseDisposable(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diagnostics := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diagnostics[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.Effect_acquireDisposable_expresses_this_disposable_resource_acquisition_more_directly_than_Effect_acquireRelease_with_a_manual_disposal_finalizer_effect_acquireReleaseDisposable,
				nil,
			)
		}
		return diagnostics
	},
}

// AcquireReleaseDisposableMatch holds the nodes needed by the diagnostic and
// its quick fix.
type AcquireReleaseDisposableMatch struct {
	SourceFile       *ast.SourceFile
	Location         core.TextRange
	CallNode         *ast.Node
	EffectModule     *ast.Node
	Acquire          *ast.Node
	HasTypeArguments bool
}

// AnalyzeAcquireReleaseDisposable finds Effect.acquireRelease calls whose
// resulting success type is disposable and whose release callback consists
// solely of invoking that resource's disposal protocol.
func AnalyzeAcquireReleaseDisposable(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []AcquireReleaseDisposableMatch {
	if tp == nil || c == nil || sf == nil || tp.SupportedEffectVersion() != typeparser.EffectMajorV4 {
		return nil
	}

	disposable := globalDisposableUnion(c)
	globalSymbol := c.GetGlobalSymbol("Symbol", ast.SymbolFlagsValue, nil)
	if disposable == nil || globalSymbol == nil {
		return nil
	}

	var matches []AcquireReleaseDisposableMatch
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}

		if match, ok := analyzeAcquireReleaseDisposableCall(tp, c, sf, node, disposable, globalSymbol); ok {
			matches = append(matches, match)
		}

		node.ForEachChild(walk)
		return false
	}

	walk(sf.AsNode())
	return matches
}

func globalDisposableUnion(c *checker.Checker) *checker.Type {
	var types []*checker.Type
	for _, name := range []string{"Disposable", "AsyncDisposable"} {
		symbol := c.GetGlobalSymbol(name, ast.SymbolFlagsType, nil)
		if symbol == nil {
			continue
		}
		if t := c.GetDeclaredTypeOfSymbol(symbol); t != nil {
			types = append(types, t)
		}
	}
	if len(types) == 0 {
		return nil
	}
	if len(types) == 1 {
		return types[0]
	}
	return c.GetUnionTypeEx(types, checker.UnionReductionNone)
}

func analyzeAcquireReleaseDisposableCall(
	tp *typeparser.TypeParser,
	c *checker.Checker,
	sf *ast.SourceFile,
	node *ast.Node,
	disposable *checker.Type,
	globalSymbol *ast.Symbol,
) (AcquireReleaseDisposableMatch, bool) {
	if node.Kind != ast.KindCallExpression {
		return AcquireReleaseDisposableMatch{}, false
	}
	call := node.AsCallExpression()
	if call == nil || call.Expression == nil || call.QuestionDotToken != nil || call.Arguments == nil || len(call.Arguments.Nodes) != 2 ||
		containsSpreadElement(call.Arguments.Nodes) || !tp.IsNodeReferenceToEffectModuleApi(call.Expression, "acquireRelease") {
		return AcquireReleaseDisposableMatch{}, false
	}

	result := tp.StrictEffectType(tp.GetTypeAtLocation(node), node)
	if result == nil || !isDefinitelyDisposable(tp, c, result.A, disposable) {
		return AcquireReleaseDisposableMatch{}, false
	}

	if !isDisposalRelease(tp, c, call.Arguments.Nodes[1], globalSymbol) {
		return AcquireReleaseDisposableMatch{}, false
	}

	var effectModule *ast.Node
	if call.Expression.Kind == ast.KindPropertyAccessExpression {
		access := call.Expression.AsPropertyAccessExpression()
		if access != nil && access.QuestionDotToken == nil {
			effectModule = access.Expression
		}
	}

	return AcquireReleaseDisposableMatch{
		SourceFile:       sf,
		Location:         scanner.GetErrorRangeForNode(sf, call.Expression),
		CallNode:         node,
		EffectModule:     effectModule,
		Acquire:          call.Arguments.Nodes[0],
		HasTypeArguments: hasCallTypeArguments(call),
	}, true
}

func isDefinitelyDisposable(tp *typeparser.TypeParser, c *checker.Checker, success *checker.Type, disposable *checker.Type) bool {
	if success == nil || disposable == nil {
		return false
	}
	for _, member := range tp.UnrollUnionMembers(success) {
		if member == nil || member.Flags()&(checker.TypeFlagsAnyOrUnknown|checker.TypeFlagsNever) != 0 {
			return false
		}
	}
	return c.IsTypeAssignableTo(success, disposable)
}

func isDisposalRelease(tp *typeparser.TypeParser, c *checker.Checker, node *ast.Node, globalSymbol *ast.Symbol) bool {
	release := typeparser.ParseLazyExpression(ast.SkipParentheses(node), false)
	if release == nil || len(release.Params) < 1 || len(release.Params) > 2 ||
		ast.GetCombinedModifierFlags(release.Node)&ast.ModifierFlagsAsync != 0 {
		return false
	}
	if release.Node.Kind == ast.KindFunctionExpression && release.Node.AsFunctionExpression().AsteriskToken != nil {
		return false
	}

	resource := release.Params[0]
	if !isPlainIdentifierParameter(resource) {
		return false
	}
	if len(release.Params) == 2 && !isPlainIdentifierParameter(release.Params[1]) {
		return false
	}
	resourceSymbol := tp.GetSymbolAtLocation(resource.Name())
	if resourceSymbol == nil {
		return false
	}

	expression := ast.SkipParentheses(release.Expression)
	if expression == nil || expression.Kind != ast.KindCallExpression {
		return false
	}
	wrapper := expression.AsCallExpression()
	if wrapper == nil || wrapper.Expression == nil || wrapper.QuestionDotToken != nil || wrapper.Arguments == nil || len(wrapper.Arguments.Nodes) != 1 ||
		containsSpreadElement(wrapper.Arguments.Nodes) {
		return false
	}

	wrapperName := ""
	switch {
	case tp.IsNodeReferenceToEffectModuleApi(wrapper.Expression, "sync"):
		wrapperName = "sync"
	case tp.IsNodeReferenceToEffectModuleApi(wrapper.Expression, "promise"):
		wrapperName = "promise"
	default:
		return false
	}

	thunk := typeparser.ParseLazyExpression(ast.SkipParentheses(wrapper.Arguments.Nodes[0]), true)
	if thunk == nil || thunk.Expression == nil {
		return false
	}
	if wrapperName == "sync" && ast.GetCombinedModifierFlags(thunk.Node)&ast.ModifierFlagsAsync != 0 {
		return false
	}
	if thunk.Node.Kind == ast.KindFunctionExpression && thunk.Node.AsFunctionExpression().AsteriskToken != nil {
		return false
	}

	protocol, ok := disposalProtocolCall(tp, c, thunk.Expression, resourceSymbol, globalSymbol)
	return ok && (protocol == "dispose" && wrapperName == "sync" || protocol == "asyncDispose" && wrapperName == "promise")
}

func isPlainIdentifierParameter(node *ast.Node) bool {
	if node == nil || node.Kind != ast.KindParameter || node.Name() == nil || node.Name().Kind != ast.KindIdentifier {
		return false
	}
	parameter := node.AsParameterDeclaration()
	return parameter != nil && parameter.DotDotDotToken == nil && parameter.QuestionToken == nil && parameter.Initializer == nil
}

func disposalProtocolCall(
	tp *typeparser.TypeParser,
	c *checker.Checker,
	node *ast.Node,
	resourceSymbol *ast.Symbol,
	globalSymbol *ast.Symbol,
) (string, bool) {
	node = ast.SkipParentheses(node)
	if node == nil || node.Kind != ast.KindCallExpression {
		return "", false
	}
	call := node.AsCallExpression()
	if call == nil || call.QuestionDotToken != nil || call.TypeArguments != nil && len(call.TypeArguments.Nodes) > 0 ||
		call.Arguments == nil || len(call.Arguments.Nodes) != 0 || call.Expression == nil || call.Expression.Kind != ast.KindElementAccessExpression {
		return "", false
	}

	element := call.Expression.AsElementAccessExpression()
	if element == nil || element.QuestionDotToken != nil || element.Expression == nil || element.ArgumentExpression == nil {
		return "", false
	}
	actualResource := tp.GetSymbolAtLocation(ast.SkipParentheses(element.Expression))
	if actualResource == nil || checker.Checker_getSymbolIfSameReference(c, actualResource, resourceSymbol) == nil {
		return "", false
	}

	key := ast.SkipParentheses(element.ArgumentExpression)
	if key == nil || key.Kind != ast.KindPropertyAccessExpression {
		return "", false
	}
	access := key.AsPropertyAccessExpression()
	if access == nil || access.QuestionDotToken != nil || access.Expression == nil || access.Name() == nil {
		return "", false
	}
	actualSymbol := tp.ResolveToGlobalSymbol(tp.GetSymbolAtLocation(access.Expression))
	if actualSymbol == nil || actualSymbol != globalSymbol {
		return "", false
	}

	protocol := access.Name().Text()
	return protocol, protocol == "dispose" || protocol == "asyncDispose"
}
