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
	Node                    *ast.Node // The expression replaced by the fix; may be inner to Flow.Node()
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

		reportedCount, reportedNode := reportableTransformationCount(c, flow)
		if reportedNode == nil {
			continue
		}

		runs := safelyPipeableRuns(tp, flow)
		for firstPipeableIndex := 0; firstPipeableIndex <= reportedCount; firstPipeableIndex++ {
			run := runs[firstPipeableIndex]
			if run.callCount < minArgCount || !isPipeableAtIndex(tp, flow, firstPipeableIndex) {
				continue
			}
			var pipeableTransformations []typeparser.PipingFlowTransformation
			for i := firstPipeableIndex; i < firstPipeableIndex+run.length; i++ {
				pipeableTransformations = append(pipeableTransformations, flow.Transformations().At(i))
			}
			pipeableEndIndex := firstPipeableIndex + len(pipeableTransformations)

			subjectText := trimmedNodeText(sf, flow.TransformationInputNode(firstPipeableIndex))

			afterTransformations := flow.Transformations().Slice(pipeableEndIndex, reportedCount)

			matches = append(matches, MissedPipeableOpportunityMatch{
				SourceFile:              sf,
				Location:                scanner.GetErrorRangeForNode(sf, reportedNode),
				Node:                    reportedNode,
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

type safelyPipeableRun struct {
	length    int
	callCount int
}

func safelyPipeableRuns(tp *typeparser.TypeParser, flow *typeparser.PipingFlow) []safelyPipeableRun {
	transformationCount := flow.Transformations().Len()
	runs := make([]safelyPipeableRun, transformationCount+1)
	for i := transformationCount - 1; i >= 0; i-- {
		t := flow.Transformations().At(i)
		if t.Kind() == typeparser.TransformationKindCallWithOptionals || !tp.IsSafelyPipeableCallee(t.Callee()) {
			continue
		}
		runs[i] = safelyPipeableRun{length: runs[i+1].length + 1, callCount: runs[i+1].callCount}
		if t.Kind() == typeparser.TransformationKindCall {
			runs[i].callCount++
		}
	}
	return runs
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

// reportableTransformationCount returns how many leading transformations of
// the flow may be reported, and the expression they produce. Flows whose
// output is callable are not reported; when that output comes after a
// callWithOptionals step, the flow is cut before that step instead, as it
// cannot join a pipe anyway. It returns a nil node when nothing is reportable.
func reportableTransformationCount(c *checker.Checker, flow *typeparser.PipingFlow) (int, *ast.Node) {
	count := flow.Transformations().Len()
	node := flow.Node()
	for count > 0 {
		outType := flow.Transformations().At(count - 1).OutType()
		if outType == nil || len(c.GetSignaturesOfType(outType, checker.SignatureKindCall)) == 0 {
			return count, node
		}
		count--
		for count >= 0 && flow.Transformations().At(count).Kind() != typeparser.TransformationKindCallWithOptionals {
			count--
		}
		if count < 0 {
			return 0, nil
		}
		node = flow.TransformationInputNode(count)
		if node == nil {
			return 0, nil
		}
	}
	return 0, nil
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
