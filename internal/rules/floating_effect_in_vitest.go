package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
)

type vitestCallbackKind uint8

const (
	vitestCallbackNone vitestCallbackKind = iota
	vitestCallbackTest
	vitestCallbackHook
)

type vitestRoot struct {
	name         string
	effectVitest bool
}

var vitestTestModifiers = map[string]struct{}{
	"concurrent": {},
	"each":       {},
	"fails":      {},
	"for":        {},
	"only":       {},
	"prop":       {},
	"runIf":      {},
	"sequential": {},
	"skip":       {},
	"skipIf":     {},
	"todo":       {},
}

var vitestHookNames = map[string]struct{}{
	"afterAll":   {},
	"afterEach":  {},
	"beforeAll":  {},
	"beforeEach": {},
}

var effectAwareVitestMethods = map[string]struct{}{
	"effect":     {},
	"live":       {},
	"scoped":     {},
	"scopedLive": {},
}

// FloatingEffectInVitest detects Effects returned from Vitest callbacks that do
// not execute returned Effect values.
var FloatingEffectInVitest = rule.Rule{
	Name:            "floatingEffectInVitest",
	Group:           "correctness",
	Description:     "Detects Effects returned from non-Effect-aware Vitest callbacks",
	DefaultSeverity: etscore.SeverityError,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.This_Vitest_callback_returns_an_Effect_that_Vitest_does_not_run_Use_an_Effect_aware_test_API_or_run_the_Effect_with_Effect_runPromise_effect_floatingEffectInVitest.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		var diags []*ast.Diagnostic
		var walk ast.Visitor
		walk = func(node *ast.Node) bool {
			if node == nil {
				return false
			}

			if node.Kind == ast.KindCallExpression {
				if callback := floatingEffectVitestCallback(ctx, node.AsCallExpression()); callback != nil {
					rangeNode := callback
					if lazy := typeparser.ParseLazyExpression(callback, false); lazy != nil && lazy.Expression != nil {
						rangeNode = lazy.Expression
					}
					diags = append(diags, ctx.NewDiagnostic(
						ctx.SourceFile,
						ctx.GetErrorRange(rangeNode),
						tsdiag.This_Vitest_callback_returns_an_Effect_that_Vitest_does_not_run_Use_an_Effect_aware_test_API_or_run_the_Effect_with_Effect_runPromise_effect_floatingEffectInVitest,
						nil,
					))
				}
			}

			node.ForEachChild(walk)
			return false
		}

		walk(ctx.SourceFile.AsNode())
		return diags
	},
}

func floatingEffectVitestCallback(ctx *rule.Context, call *ast.CallExpression) *ast.Node {
	if call == nil || call.Expression == nil || call.Arguments == nil {
		return nil
	}

	root, members := matchVitestCallee(ctx.TypeParser, call.Expression)
	callbackKind := classifyVitestCallback(root, members)
	if callbackKind == vitestCallbackNone {
		return nil
	}

	start := 1
	if callbackKind == vitestCallbackHook {
		start = 0
	}
	for i := start; i < len(call.Arguments.Nodes); i++ {
		callback := call.Arguments.Nodes[i]
		if vitestCallbackReturnsEffect(ctx.Checker, ctx.TypeParser, callback) {
			return callback
		}
	}
	return nil
}

func matchVitestCallee(tp *typeparser.TypeParser, node *ast.Node) (vitestRoot, []string) {
	var reversedMembers []string
	for node != nil {
		node = ast.SkipParentheses(node)
		if root := matchVitestRoot(tp, node); root.name != "" {
			members := make([]string, len(reversedMembers))
			for i, member := range reversedMembers {
				members[len(reversedMembers)-1-i] = member
			}
			return root, members
		}
		switch node.Kind {
		case ast.KindPropertyAccessExpression:
			property := node.AsPropertyAccessExpression()
			if property == nil || property.Name() == nil {
				return vitestRoot{}, nil
			}
			reversedMembers = append(reversedMembers, property.Name().Text())
			node = property.Expression
		case ast.KindCallExpression:
			call := node.AsCallExpression()
			if call == nil {
				return vitestRoot{}, nil
			}
			node = call.Expression
		case ast.KindTaggedTemplateExpression:
			tagged := node.AsTaggedTemplateExpression()
			if tagged == nil {
				return vitestRoot{}, nil
			}
			node = tagged.Tag
		default:
			return vitestRoot{}, nil
		}
	}
	return vitestRoot{}, nil
}

func matchVitestRoot(tp *typeparser.TypeParser, node *ast.Node) vitestRoot {
	for _, name := range []string{"it", "test"} {
		if tp.IsNodeReferenceToVitestApi(node, name) {
			return vitestRoot{name: name}
		}
	}
	if tp.IsNodeReferenceToEffectVitestApi(node, "it") {
		return vitestRoot{name: "it", effectVitest: true}
	}
	for name := range vitestHookNames {
		if tp.IsNodeReferenceToVitestApi(node, name) {
			return vitestRoot{name: name}
		}
	}
	return vitestRoot{}
}

func classifyVitestCallback(root vitestRoot, members []string) vitestCallbackKind {
	if root.name == "" {
		return vitestCallbackNone
	}
	if _, isHook := vitestHookNames[root.name]; isHook {
		if len(members) == 0 {
			return vitestCallbackHook
		}
		return vitestCallbackNone
	}

	if root.effectVitest && len(members) > 0 {
		if _, effectAware := effectAwareVitestMethods[members[0]]; effectAware {
			return vitestCallbackNone
		}
	}
	if len(members) == 1 {
		if _, isHook := vitestHookNames[members[0]]; isHook {
			return vitestCallbackHook
		}
	}
	for _, member := range members {
		if _, isModifier := vitestTestModifiers[member]; !isModifier {
			return vitestCallbackNone
		}
	}
	return vitestCallbackTest
}

func vitestCallbackReturnsEffect(c *checker.Checker, tp *typeparser.TypeParser, callback *ast.Node) bool {
	callbackType := tp.GetTypeAtLocation(callback)
	if callbackType == nil {
		return false
	}
	for _, member := range tp.UnrollUnionMembers(callbackType) {
		for _, signature := range c.GetSignaturesOfType(member, checker.SignatureKindCall) {
			if vitestReturnTypeContainsEffect(c, tp, c.GetReturnTypeOfSignature(signature), callback, 0) {
				return true
			}
		}
	}
	if lazy := typeparser.ParseLazyExpression(callback, false); lazy != nil && lazy.Expression != nil {
		return vitestReturnTypeContainsEffect(c, tp, tp.GetTypeAtLocation(lazy.Expression), lazy.Expression, 0)
	}
	return false
}

func vitestReturnTypeContainsEffect(c *checker.Checker, tp *typeparser.TypeParser, returnType *checker.Type, atLocation *ast.Node, depth int) bool {
	for _, member := range tp.UnrollUnionMembers(returnType) {
		if tp.StrictIsEffectType(member, atLocation) {
			return true
		}
		if depth == 0 && tp.PromiseType(member) != nil {
			for _, typeArg := range c.GetTypeArguments(member) {
				if vitestReturnTypeContainsEffect(c, tp, typeArg, atLocation, depth+1) {
					return true
				}
			}
		}
	}
	return false
}
