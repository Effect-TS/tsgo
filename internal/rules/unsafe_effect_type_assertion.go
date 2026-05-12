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

var UnsafeEffectTypeAssertion = rule.Rule{
	Name:            "unsafeEffectTypeAssertion",
	Group:           "effectNative",
	Description:     "Detects unsafe type assertions that narrow Effect, Stream, or Layer error or requirements channels",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.This_type_assertion_unsafely_narrows_the_error_or_requirements_channels_effect_unsafeEffectTypeAssertion.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeUnsafeEffectTypeAssertion(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diags[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.This_type_assertion_unsafely_narrows_the_error_or_requirements_channels_effect_unsafeEffectTypeAssertion,
				unsafeEffectTypeAssertionRelatedInformation(ctx, match),
			)
		}
		return diags
	},
}

type UnsafeEffectTypeAssertionChannel struct {
	Name     string
	Original string
	Asserted string
}

type UnsafeEffectTypeAssertionMatch struct {
	SourceFile     *ast.SourceFile
	Location       core.TextRange
	AssertionNode  *ast.Node
	ExpressionNode *ast.Node
	LocationNode   *ast.Node
	Channels       []UnsafeEffectTypeAssertionChannel
}

func AnalyzeUnsafeEffectTypeAssertion(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []UnsafeEffectTypeAssertionMatch {
	if tp == nil || c == nil || sf == nil {
		return nil
	}

	var matches []UnsafeEffectTypeAssertionMatch
	parseEffectStreamOrLayer := func(t *checker.Type, atLocation *ast.Node) (e *checker.Type, r *checker.Type, ok bool) {
		if effect := tp.EffectType(t, atLocation); effect != nil {
			return effect.E, effect.R, true
		}
		if stream := tp.StreamType(t, atLocation); stream != nil {
			return stream.E, stream.R, true
		}
		if layer := tp.LayerType(t, atLocation); layer != nil {
			return layer.E, layer.RIn, true
		}
		return nil, nil, false
	}
	nodesToVisit := make([]*ast.Node, 0)
	pushChild := func(child *ast.Node) bool {
		nodesToVisit = append(nodesToVisit, child)
		return false
	}
	sf.AsNode().ForEachChild(pushChild)

	for len(nodesToVisit) > 0 {
		node := nodesToVisit[len(nodesToVisit)-1]
		nodesToVisit = nodesToVisit[:len(nodesToVisit)-1]
		node.ForEachChild(pushChild)

		if node.Kind != ast.KindAsExpression && node.Kind != ast.KindTypeAssertionExpression {
			continue
		}

		expr := node.Expression()
		if expr == nil {
			continue
		}

		originalType := tp.GetTypeAtLocation(expr)
		assertedType := tp.GetTypeAtLocation(node)
		if originalType == nil || assertedType == nil {
			continue
		}

		originalE, originalR, ok := parseEffectStreamOrLayer(originalType, expr)
		if !ok {
			continue
		}

		assertedE, assertedR, ok := parseEffectStreamOrLayer(assertedType, node)
		if !ok {
			continue
		}

		channels := make([]UnsafeEffectTypeAssertionChannel, 0, 2)
		if originalE != nil && assertedE != nil && !isAnyType(originalE) && !checker.Checker_isTypeAssignableTo(c, originalE, assertedE) {
			channels = append(channels, UnsafeEffectTypeAssertionChannel{
				Name:     "error",
				Original: c.TypeToString(originalE),
				Asserted: c.TypeToString(assertedE),
			})
		}
		if originalR != nil && assertedR != nil && !isAnyType(originalR) && !checker.Checker_isTypeAssignableTo(c, originalR, assertedR) {
			channels = append(channels, UnsafeEffectTypeAssertionChannel{
				Name:     "requirements",
				Original: c.TypeToString(originalR),
				Asserted: c.TypeToString(assertedR),
			})
		}
		if len(channels) == 0 {
			continue
		}

		locationNode := node
		if typeNode := node.Type(); typeNode != nil {
			locationNode = typeNode
		}

		matches = append(matches, UnsafeEffectTypeAssertionMatch{
			SourceFile:     sf,
			Location:       scanner.GetErrorRangeForNode(sf, node),
			AssertionNode:  node,
			ExpressionNode: expr,
			LocationNode:   locationNode,
			Channels:       channels,
		})
	}

	return matches
}

func isAnyType(t *checker.Type) bool {
	return t != nil && t.Flags()&checker.TypeFlagsAny != 0
}

func unsafeEffectTypeAssertionRelatedInformation(ctx *rule.Context, match UnsafeEffectTypeAssertionMatch) []*ast.Diagnostic {
	if ctx == nil || len(match.Channels) == 0 {
		return nil
	}

	locationNode := match.LocationNode
	if locationNode == nil {
		locationNode = match.AssertionNode
	}

	related := make([]*ast.Diagnostic, 0, len(match.Channels))
	for _, channel := range match.Channels {
		related = append(related, ctx.NewDiagnostic(
			match.SourceFile,
			scanner.GetErrorRangeForNode(match.SourceFile, locationNode),
			tsdiag.The_0_channel_is_narrowed_from_1_to_2_effect_unsafeEffectTypeAssertion,
			nil,
			channel.Name,
			channel.Original,
			channel.Asserted,
		))
	}
	return related
}
