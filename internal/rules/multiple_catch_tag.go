package rules

import (
	"strconv"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

var MultipleCatchTag = rule.Rule{
	Name:            "multipleCatchTag",
	Group:           "style",
	Description:     "Suggests collapsing consecutive Effect.catchTag transformations into a single Effect.catchTags call when semantics stay equivalent",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v4"},
	Codes: []int32{
		tsdiag.These_0_consecutive_catchTag_transformations_can_be_collapsed_into_a_single_catchTags_call_effect_multipleCatchTag.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeMultipleCatchTag(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diags[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.These_0_consecutive_catchTag_transformations_can_be_collapsed_into_a_single_catchTags_call_effect_multipleCatchTag,
				multipleCatchTagRelatedInformation(ctx, match),
				strconv.Itoa(len(match.CatchNodes)),
			)
		}
		return diags
	},
}

type MultipleCatchTagMatch struct {
	SourceFile *ast.SourceFile
	Location   core.TextRange
	Node       *ast.Node
	CatchNodes []*ast.Node
}

type multipleCatchTagCandidate struct {
	transformation *typeparser.PipingFlowTransformation
	handledTagType *checker.Type
	introducedTags []*checker.Type
}

func AnalyzeMultipleCatchTag(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []MultipleCatchTagMatch {
	if tp == nil || c == nil || sf == nil {
		return nil
	}

	var matches []MultipleCatchTagMatch
	for _, flow := range tp.PipingFlows(sf, true) {
		var chain []multipleCatchTagCandidate
		var introducedTags []*checker.Type

		flush := func() {
			if len(chain) >= 2 {
				matches = append(matches, MultipleCatchTagMatch{
					SourceFile: sf,
					Location:   scanner.GetErrorRangeForNode(sf, chain[0].transformation.Callee),
					Node:       chain[0].transformation.Callee,
					CatchNodes: collectMultipleCatchTagNodes(chain),
				})
			}
			chain = nil
			introducedTags = nil
		}

		for i := range flow.Transformations {
			transformation := &flow.Transformations[i]
			if !tp.IsNodeReferenceToEffectPackageExport(transformation.Callee, "catchTag") {
				flush()
				continue
			}

			candidate, ok := analyzeMultipleCatchTagCandidate(tp, c, transformation)
			if !ok {
				flush()
				continue
			}

			if overlapsAnyTagType(c, candidate.handledTagType, introducedTags) {
				flush()
			}

			chain = append(chain, candidate)
			introducedTags = append(introducedTags, candidate.introducedTags...)
		}

		flush()
	}

	return matches
}

func analyzeMultipleCatchTagCandidate(tp *typeparser.TypeParser, c *checker.Checker, transformation *typeparser.PipingFlowTransformation) (multipleCatchTagCandidate, bool) {
	if tp == nil || c == nil || transformation == nil || transformation.Callee == nil || len(transformation.Args) != 2 {
		return multipleCatchTagCandidate{}, false
	}

	tagArg := unwrapTransparentExpression(transformation.Args[0])
	if tagArg == nil || !ast.IsStringLiteral(tagArg) {
		return multipleCatchTagCandidate{}, false
	}

	handledTagType := tp.GetTypeAtLocation(tagArg)
	if handledTagType == nil {
		return multipleCatchTagCandidate{}, false
	}

	handlerType := tp.GetTypeAtLocation(transformation.Args[1])
	if handlerType == nil {
		return multipleCatchTagCandidate{}, false
	}

	signatures := c.GetSignaturesOfType(handlerType, checker.SignatureKindCall)
	if len(signatures) != 1 {
		return multipleCatchTagCandidate{}, false
	}

	returnType := c.GetReturnTypeOfSignature(signatures[0])
	if returnType == nil {
		return multipleCatchTagCandidate{}, false
	}

		errorChannel := catchTagReturnErrorChannel(tp, returnType, transformation.Args[1])
		if errorChannel == nil {
			return multipleCatchTagCandidate{}, false
		}

	return multipleCatchTagCandidate{
		transformation: transformation,
		handledTagType: handledTagType,
		introducedTags: collectErrorTagTypes(tp, c, errorChannel),
	}, true
}

func catchTagReturnErrorChannel(tp *typeparser.TypeParser, returnType *checker.Type, atLocation *ast.Node) *checker.Type {
	if tp == nil || returnType == nil {
		return nil
	}
	if effectType := tp.EffectType(returnType, atLocation); effectType != nil {
		return effectType.E
	}
	if streamType := tp.StreamType(returnType, atLocation); streamType != nil {
		return streamType.E
	}
	return nil
}

func collectErrorTagTypes(tp *typeparser.TypeParser, c *checker.Checker, errorType *checker.Type) []*checker.Type {
	var tags []*checker.Type
	if tp == nil || c == nil || errorType == nil {
		return tags
	}

	for _, member := range tp.UnrollUnionMembers(errorType) {
		if member == nil || member.Flags()&checker.TypeFlagsNever != 0 {
			continue
		}

		tagType := c.GetTypeOfPropertyOfType(member, "_tag")
		if tagType == nil {
			tagType = tp.GetTypeOfPropertyByName(member, "_tag")
		}
		if tagType == nil {
			continue
		}
		tags = append(tags, tagType)
	}

	return tags
}

func overlapsAnyTagType(c *checker.Checker, current *checker.Type, previous []*checker.Type) bool {
	if c == nil || current == nil {
		return false
	}

	for _, prior := range previous {
		if prior == nil {
			continue
		}
		if c.IsTypeAssignableTo(current, prior) && c.IsTypeAssignableTo(prior, current) {
			return true
		}
	}

	return false
}

func collectMultipleCatchTagNodes(chain []multipleCatchTagCandidate) []*ast.Node {
	nodes := make([]*ast.Node, 0, len(chain))
	for _, candidate := range chain {
		if candidate.transformation == nil || candidate.transformation.Callee == nil {
			continue
		}
		nodes = append(nodes, candidate.transformation.Callee)
	}
	return nodes
}

func multipleCatchTagRelatedInformation(ctx *rule.Context, match MultipleCatchTagMatch) []*ast.Diagnostic {
	if ctx == nil || len(match.CatchNodes) < 2 {
		return nil
	}

	related := make([]*ast.Diagnostic, 0, len(match.CatchNodes)-1)
	for _, node := range match.CatchNodes[1:] {
		if node == nil {
			continue
		}
		related = append(related, ctx.NewDiagnostic(
			match.SourceFile,
			scanner.GetErrorRangeForNode(match.SourceFile, node),
			tsdiag.This_catchTag_transformation_is_part_of_a_consecutive_chain_that_can_be_collapsed_into_catchTags_effect_multipleCatchTag,
			nil,
		))
	}
	return related
}
