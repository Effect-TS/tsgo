package rules

import (
	"strings"

	"github.com/effect-ts/tsgo/internal/readonly"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"

	"github.com/effect-ts/tsgo/etscore"
)

// MissedPipeableOpportunity detects nested function call chains that can be converted to .pipe() style.
var MissedPipeableOpportunity = rule.Rule{
	Name:            "missedPipeableOpportunity",
	Group:           "style",
	Description:     "Suggests using .pipe() for nested function calls",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.This_nested_call_structure_has_a_pipeable_form_0_pipe_represents_the_same_call_sequence_in_pipe_style_and_may_be_easier_to_read_effect_missedPipeableOpportunity.Code()},
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
				tsdiag.This_nested_call_structure_has_a_pipeable_form_0_pipe_represents_the_same_call_sequence_in_pipe_style_and_may_be_easier_to_read_effect_missedPipeableOpportunity,
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
	AfterTransformations    readonly.Slice[typeparser.PipingFlowTransformation]
}

// AnalyzeMissedPipeableOpportunity finds all nested call chains that can be converted to .pipe() style.
func AnalyzeMissedPipeableOpportunity(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile, minArgCount int) []MissedPipeableOpportunityMatch {
	flows := tp.PipingFlows(sf, false)

	var matches []MissedPipeableOpportunityMatch

	for _, flow := range flows.All() {
		// Skip flows with too few transformations
		if flow.Transformations().Len() < minArgCount {
			continue
		}

		// Skip if final output type is callable (has call signatures)
		lastTransformation := flow.Transformations().At(flow.Transformations().Len() - 1)
		if lastTransformation.OutType() != nil {
			callSigs := c.GetSignaturesOfType(lastTransformation.OutType(), checker.SignatureKindCall)
			if len(callSigs) > 0 {
				continue
			}
		}

		// For each start index, the length of the run of safely pipeable callees
		// and the number of call steps in it. A segment is reported at the first
		// index whose run has enough call steps and whose type is pipeable, so
		// the type is only computed where the run qualifies.
		transformationCount := flow.Transformations().Len()
		runLength := make([]int, transformationCount+1)
		runCallCount := make([]int, transformationCount+1)
		for i := transformationCount - 1; i >= 0; i-- {
			t := flow.Transformations().At(i)
			if !tp.IsSafelyPipeableCallee(t.Callee()) {
				continue
			}
			runLength[i] = runLength[i+1] + 1
			runCallCount[i] = runCallCount[i+1]
			if t.Kind() == typeparser.TransformationKindCall {
				runCallCount[i]++
			}
		}

		for firstPipeableIndex := 0; firstPipeableIndex <= transformationCount; firstPipeableIndex++ {
			if runCallCount[firstPipeableIndex] < minArgCount || !isPipeableAtIndex(tp, flow, firstPipeableIndex) {
				continue
			}
			var pipeableTransformations []typeparser.PipingFlowTransformation
			for i := firstPipeableIndex; i < firstPipeableIndex+runLength[firstPipeableIndex]; i++ {
				pipeableTransformations = append(pipeableTransformations, flow.Transformations().At(i))
			}
			pipeableEndIndex := firstPipeableIndex + len(pipeableTransformations)

			// Get subject text for the diagnostic message
			subjectText := getSubjectText(c, sf, flow, firstPipeableIndex)

			afterTransformations := flow.Transformations().Slice(pipeableEndIndex, flow.Transformations().Len())

			matches = append(matches, MissedPipeableOpportunityMatch{
				SourceFile:              sf,
				Location:                scanner.GetErrorRangeForNode(sf, flow.Node()),
				PipeableStartIndex:      firstPipeableIndex,
				PipeableTransformations: pipeableTransformations,
				Flow:                    flow,
				SubjectText:             subjectText,
				AfterTransformations:    afterTransformations,
			})

			// Found and reported a valid segment, move past it (no overlapping diagnostics)
			break
		}
	}

	return matches
}

// isPipeableAtIndex checks if the type at a given index in a flow is pipeable.
// Index 0 = subject, index > 0 = transformations[index - 1].outType
func isPipeableAtIndex(tp *typeparser.TypeParser, flow *typeparser.PipingFlow, index int) bool {
	if index == 0 {
		if tp.LiteralCannotBePipeable(flow.Subject().Node()) {
			return false
		}
		subjectType := flow.Subject().OutType()
		if subjectType == nil {
			return false
		}
		return tp.IsPipeableType(subjectType)
	}

	t := flow.Transformations().At(index - 1)
	if t.OutType() == nil {
		return false
	}
	return tp.IsPipeableType(t.OutType())
}

// getSubjectText extracts the subject text for the diagnostic message.
// If the pipeable segment starts at index 0, uses the subject node text directly.
// Otherwise, traverses the flow node to find the node at the right depth.
func getSubjectText(_ *checker.Checker, sf *ast.SourceFile, flow *typeparser.PipingFlow, firstPipeableIndex int) string {
	if firstPipeableIndex == 0 {
		return trimmedNodeText(sf, flow.Subject().Node())
	}

	// Traverse from flow.node into arguments to find the node at the right depth
	current := flow.Node()
	for i := flow.Transformations().Len(); i > firstPipeableIndex; i-- {
		t := flow.Transformations().At(i - 1)
		if t.Kind() == typeparser.TransformationKindCall && ast.IsCallExpression(current) {
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
