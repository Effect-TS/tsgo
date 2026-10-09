package rules

import (
	"math/big"
	"regexp"
	"strconv"
	"strings"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

var EffectSleepZeroToYieldNow = rule.Rule{
	Name:            "effectSleepZeroToYieldNow",
	Group:           "style",
	Description:     "Suggests Effect.yieldNow for cooperative yielding instead of sleeping for zero duration",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Consider_Effect_yieldNow_for_cooperative_yielding_instead_of_sleeping_for_zero_duration_effect_effectSleepZeroToYieldNow.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeEffectSleepZeroToYieldNow(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diags[i] = ctx.NewDiagnostic(match.SourceFile, match.Location,
				tsdiag.Consider_Effect_yieldNow_for_cooperative_yielding_instead_of_sleeping_for_zero_duration_effect_effectSleepZeroToYieldNow, nil)
		}
		return diags
	},
}

type YieldNowForm uint8

const (
	YieldNowCall YieldNowForm = iota + 1
	YieldNowValue
)

type EffectSleepZeroToYieldNowMatch struct {
	SourceFile   *ast.SourceFile
	Location     core.TextRange
	Call         *ast.Node
	EffectModule *ast.Node
	Form         YieldNowForm
}

func AnalyzeEffectSleepZeroToYieldNow(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile) []EffectSleepZeroToYieldNowMatch {
	version := tp.DetectEffectVersion()
	var form YieldNowForm
	switch version {
	case typeparser.EffectMajorV3:
		form = YieldNowCall
	case typeparser.EffectMajorV4:
		form = YieldNowValue
	default:
		return nil
	}

	var matches []EffectSleepZeroToYieldNowMatch
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node.Kind == ast.KindCallExpression && isOrdinarySingleArgumentCall(node) {
			call := node.AsCallExpression()
			callee := unwrapZeroDurationExpression(call.Expression)
			if callee.Flags&ast.NodeFlagsOptionalChain == 0 &&
				tp.IsNodeReferenceToEffectModuleApi(callee, "sleep") &&
				isRemovableZeroDuration(tp, call.Arguments.Nodes[0], version) {
				var receiver *ast.Node
				if callee.Kind == ast.KindPropertyAccessExpression {
					candidate := callee.AsPropertyAccessExpression().Expression
					if isImportedModuleReceiver(tp, candidate) && tp.IsExpressionEffectModule(unwrapZeroDurationExpression(candidate)) {
						receiver = candidate
					}
				}
				matches = append(matches, EffectSleepZeroToYieldNowMatch{
					SourceFile:   sf,
					Location:     scanner.GetErrorRangeForNode(sf, call.Expression),
					Call:         node,
					EffectModule: receiver,
					Form:         form,
				})
			}
		}
		node.ForEachChild(walk)
		return false
	}
	walk(sf.AsNode())
	return matches
}

func unwrapZeroDurationExpression(node *ast.Node) *ast.Node {
	return ast.SkipOuterExpressions(node, ast.OEKParentheses|ast.OEKTypeAssertions|ast.OEKSatisfies)
}

func isOrdinarySingleArgumentCall(node *ast.Node) bool {
	call := node.AsCallExpression()
	return node.Flags&ast.NodeFlagsOptionalChain == 0 && call.Expression != nil &&
		call.TypeArguments == nil && call.Arguments != nil && len(call.Arguments.Nodes) == 1 &&
		call.Arguments.Nodes[0].Kind != ast.KindSpreadElement
}

func isImportedModuleReceiver(tp *typeparser.TypeParser, node *ast.Node) bool {
	node = unwrapZeroDurationExpression(node)
	if node.Kind == ast.KindPropertyAccessExpression && node.Flags&ast.NodeFlagsOptionalChain == 0 {
		root := unwrapZeroDurationExpression(node.AsPropertyAccessExpression().Expression)
		if root.Kind != ast.KindIdentifier {
			return false
		}
		symbol := tp.GetSymbolAtLocation(root)
		return symbol != nil && len(symbol.Declarations) == 1 && symbol.Declarations[0].Kind == ast.KindNamespaceImport
	}
	if node.Kind != ast.KindIdentifier {
		return false
	}
	symbol := tp.GetSymbolAtLocation(node)
	if symbol == nil || len(symbol.Declarations) != 1 {
		return false
	}
	kind := symbol.Declarations[0].Kind
	return kind == ast.KindImportSpecifier || kind == ast.KindNamespaceImport
}

var zeroDurationString = regexp.MustCompile(`^(-?0+(?:\.0+)?)[ \t]+(nanos?|micros?|millis?|seconds?|minutes?|hours?|days?|weeks?)$`)

func isRemovableZeroDuration(tp *typeparser.TypeParser, node *ast.Node, version typeparser.EffectMajorVersion) bool {
	node = unwrapZeroDurationExpression(node)
	if isZeroDurationNumber(node, false) || isZeroDurationNumber(node, true) {
		return true
	}
	switch node.Kind {
	case ast.KindStringLiteral, ast.KindNoSubstitutionTemplateLiteral:
		parts := zeroDurationString.FindStringSubmatch(node.Text())
		if parts == nil {
			return false
		}
		return version != typeparser.EffectMajorV3 || !strings.Contains(parts[1], ".") ||
			(!strings.HasPrefix(parts[2], "nano") && !strings.HasPrefix(parts[2], "micro"))
	case ast.KindArrayLiteralExpression:
		elements := node.AsArrayLiteralExpression().Elements
		return elements != nil && len(elements.Nodes) == 2 &&
			isZeroDurationNumber(elements.Nodes[0], false) && isZeroDurationNumber(elements.Nodes[1], false)
	case ast.KindPropertyAccessExpression:
		receiver := node.AsPropertyAccessExpression().Expression
		return node.Flags&ast.NodeFlagsOptionalChain == 0 &&
			isImportedModuleReceiver(tp, receiver) && tp.IsExpressionEffectDurationModule(unwrapZeroDurationExpression(receiver)) &&
			tp.IsNodeReferenceToEffectDurationModuleApi(node, "zero")
	case ast.KindCallExpression:
		if !isOrdinarySingleArgumentCall(node) {
			return false
		}
		call := node.AsCallExpression()
		callee := unwrapZeroDurationExpression(call.Expression)
		if callee.Kind != ast.KindPropertyAccessExpression || callee.Flags&ast.NodeFlagsOptionalChain != 0 {
			return false
		}
		receiver := callee.AsPropertyAccessExpression().Expression
		if !isImportedModuleReceiver(tp, receiver) || !tp.IsExpressionEffectDurationModule(unwrapZeroDurationExpression(receiver)) {
			return false
		}
		unit := callee.AsPropertyAccessExpression().Name().Text()
		switch unit {
		case "nanos", "micros":
			return tp.IsNodeReferenceToEffectDurationModuleApi(callee, unit) && isZeroDurationNumber(call.Arguments.Nodes[0], true)
		case "millis", "seconds", "minutes", "hours", "days", "weeks":
			return tp.IsNodeReferenceToEffectDurationModuleApi(callee, unit) && isZeroDurationNumber(call.Arguments.Nodes[0], false)
		}
	}
	return false
}

func isZeroDurationNumber(node *ast.Node, bigint bool) bool {
	node = unwrapZeroDurationExpression(node)
	if node.Kind == ast.KindPrefixUnaryExpression {
		unary := node.AsPrefixUnaryExpression()
		if unary.Operator != ast.KindMinusToken && (bigint || unary.Operator != ast.KindPlusToken) {
			return false
		}
		node = unwrapZeroDurationExpression(unary.Operand)
	}
	if bigint {
		if node.Kind != ast.KindBigIntLiteral {
			return false
		}
		integer, ok := new(big.Int).SetString(strings.TrimSuffix(strings.ReplaceAll(node.Text(), "_", ""), "n"), 0)
		return ok && integer.Sign() == 0
	}
	if node.Kind != ast.KindNumericLiteral {
		return false
	}
	number, err := strconv.ParseFloat(node.Text(), 64)
	return err == nil && number == 0
}
