package rules

import (
	"strings"

	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"

	"github.com/effect-ts/tsgo/etscore"
)

// MissedPipeableOpportunity detects nested function call chains that can be converted to .pipe() style.
var MissedPipeableOpportunity = rule.Rule{
	Name:            "missedPipeableOpportunity",
	Group:           "style",
	Description:     "Suggests using .pipe() for nested function calls",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.Nested_function_calls_can_be_converted_to_pipeable_style_for_better_readability_consider_using_0_pipe_instead_effect_missedPipeableOpportunity.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		minArgCount := 2
		effectConfig := ctx.Options
		if effectConfig != nil {
			minArgCount = effectConfig.GetPipeableMinArgCount()
		}

		matches := AnalyzeMissedPipeableOpportunity(ctx.TypeParser, ctx.Checker, ctx.SourceFile, minArgCount)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			diags[i] = ctx.NewDiagnostic(
				m.SourceFile,
				m.Location,
				tsdiag.Nested_function_calls_can_be_converted_to_pipeable_style_for_better_readability_consider_using_0_pipe_instead_effect_missedPipeableOpportunity,
				nil,
				m.SubjectText,
			)
		}
		return diags
	},
}

// MissedPipeableOpportunityMatch holds the parsed result needed by both the diagnostic rule
// and the quick-fix for the missedPipeableOpportunity pattern.
type MissedPipeableOpportunityMatch struct {
	SourceFile              *ast.SourceFile
	Location                core.TextRange
	PipeableStartIndex      int
	PipeableTransformations []typeparser.PipingFlowTransformation
	Flow                    *typeparser.PipingFlow
	SubjectText             string
	AfterTransformations    []typeparser.PipingFlowTransformation
}

// AnalyzeMissedPipeableOpportunity finds all nested call chains that can be converted to .pipe() style.
func AnalyzeMissedPipeableOpportunity(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile, minArgCount int) []MissedPipeableOpportunityMatch {
	flows := tp.PipingFlows(sf, false)

	var matches []MissedPipeableOpportunityMatch

	for _, flow := range flows {
		// Skip flows with too few transformations
		if len(flow.Transformations) < minArgCount {
			continue
		}

		// Skip if final output type is callable (has call signatures)
		lastTransformation := flow.Transformations[len(flow.Transformations)-1]
		if lastTransformation.OutType != nil {
			callSigs := c.GetSignaturesOfType(lastTransformation.OutType, checker.SignatureKindCall)
			if len(callSigs) > 0 {
				continue
			}
		}

		// Search for valid pipeable segments
		searchStartIndex := 0

		for searchStartIndex <= len(flow.Transformations) {
			// Find the first pipeable type starting from searchStartIndex
			firstPipeableIndex := -1

			for i := searchStartIndex; i <= len(flow.Transformations); i++ {
				if isPipeableAtIndex(tp, flow, i) {
					firstPipeableIndex = i
					break
				}
			}

			if firstPipeableIndex == -1 {
				break
			}

			// Collect transformations while their callees are safely pipeable
			var pipeableTransformations []typeparser.PipingFlowTransformation

			for i := firstPipeableIndex; i < len(flow.Transformations); i++ {
				t := flow.Transformations[i]
				if !tp.IsSafelyPipeableCallee(t.Callee) {
					break
				}
				pipeableTransformations = append(pipeableTransformations, t)
			}

			// Count "call" kind transformations
			callKindCount := 0
			for _, t := range pipeableTransformations {
				if t.Kind == typeparser.TransformationKindCall {
					callKindCount++
				}
			}

			if callKindCount >= minArgCount {
				pipeableEndIndex := firstPipeableIndex + len(pipeableTransformations)

				// Get subject text for the diagnostic message
				subjectText := getSubjectText(c, sf, flow, firstPipeableIndex)

				afterTransformations := flow.Transformations[pipeableEndIndex:]

				matches = append(matches, MissedPipeableOpportunityMatch{
					SourceFile:              sf,
					Location:                scanner.GetErrorRangeForNode(sf, flow.Node),
					PipeableStartIndex:      firstPipeableIndex,
					PipeableTransformations: pipeableTransformations,
					Flow:                    flow,
					SubjectText:             subjectText,
					AfterTransformations:    afterTransformations,
				})

				// Found and reported a valid segment, move past it (no overlapping diagnostics)
				break
			}

			// Not enough transformations, try starting from the next position
			searchStartIndex = firstPipeableIndex + len(pipeableTransformations) + 1
		}
	}

	return matches
}

// isPipeableAtIndex checks if the type at a given index in a flow is pipeable.
// Index 0 = subject, index > 0 = transformations[index - 1].outType
func isPipeableAtIndex(tp *typeparser.TypeParser, flow *typeparser.PipingFlow, index int) bool {
	if index == 0 {
		subjectType := flow.Subject.OutType
		if subjectType == nil {
			return false
		}
		return tp.IsPipeableType(subjectType, flow.Subject.Node)
	}

	t := flow.Transformations[index-1]
	if t.OutType == nil {
		return false
	}
	return tp.IsPipeableType(t.OutType, flow.Node)
}

// getSubjectText extracts the subject text for the diagnostic message.
// If the pipeable segment starts at index 0, uses the subject node text directly.
// Otherwise, traverses the flow node to find the node at the right depth.
func getSubjectText(_ *checker.Checker, sf *ast.SourceFile, flow *typeparser.PipingFlow, firstPipeableIndex int) string {
	if firstPipeableIndex == 0 {
		return trimmedNodeText(sf, flow.Subject.Node)
	}

	// Traverse from flow.node into arguments to find the node at the right depth
	current := flow.Node
	for i := len(flow.Transformations); i > firstPipeableIndex; i-- {
		t := flow.Transformations[i-1]
		if t.Kind == typeparser.TransformationKindCall && ast.IsCallExpression(current) {
			call := current.AsCallExpression()
			if call.Arguments != nil && len(call.Arguments.Nodes) > 0 {
				current = call.Arguments.Nodes[0]
			} else {
				return ""
			}
		} else {
			return ""
		}
	}
	return trimmedNodeText(sf, current)
}

// trimmedNodeText extracts the source text of a node, trimmed of leading trivia.
func trimmedNodeText(sf *ast.SourceFile, node *ast.Node) string {
	if node == nil || sf == nil {
		return ""
	}
	text := sf.Text()
	pos := scanner.GetTokenPosOfNode(node, sf, false)
	end := node.End()
	if pos >= 0 && end >= pos && end <= len(text) {
		return strings.TrimSpace(text[pos:end])
	}
	return ""
}
