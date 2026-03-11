// Package rules contains all Effect diagnostic rule implementations.
package rules

import (
	"github.com/effect-ts/effect-typescript-go/etscore"
	"github.com/effect-ts/effect-typescript-go/internal/rule"
	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// UnnecessaryPipeChain detects chained pipe() and .pipe() calls that can
// be simplified to a single pipe call.
var UnnecessaryPipeChain = rule.Rule{
	Name:        "unnecessaryPipeChain",
	Description:     "Simplifies chained pipe calls into a single pipe call",
	DefaultSeverity: etscore.SeveritySuggestion,
	Codes:       []int32{tsdiag.Chained_pipe_calls_can_be_simplified_to_a_single_pipe_call_effect_unnecessaryPipeChain.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeUnnecessaryPipeChain(ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			diags[i] = ctx.NewDiagnostic(m.Location, tsdiag.Chained_pipe_calls_can_be_simplified_to_a_single_pipe_call_effect_unnecessaryPipeChain, nil)
		}
		return diags
	},
}

// UnnecessaryPipeChainMatch holds the diagnostic and parsed pipe call results
// needed by both the diagnostic rule and the quick-fix.
type UnnecessaryPipeChainMatch struct {
	Location   core.TextRange                 // The pre-computed error range for this match
	Outer      *typeparser.ParsedPipeCallResult // The outer pipe call parse result
	Inner      *typeparser.ParsedPipeCallResult // The inner pipe call parse result (subject of outer)
}

// AnalyzeUnnecessaryPipeChain finds all chained pipe() and .pipe() calls
// (outer pipe whose subject is also a pipe call), returning matches with
// the diagnostic and both parsed results.
func AnalyzeUnnecessaryPipeChain(c *checker.Checker, sf *ast.SourceFile) []UnnecessaryPipeChainMatch {
	var matches []UnnecessaryPipeChainMatch

	var walk func(n *ast.Node)
	walk = func(n *ast.Node) {
		if n == nil {
			return
		}

		if n.Kind == ast.KindCallExpression {
			if result := typeparser.ParsePipeCall(c, n); result != nil {
				if inner := typeparser.ParsePipeCall(c, result.Subject); inner != nil {
					matches = append(matches, UnnecessaryPipeChainMatch{
						Location: scanner.GetErrorRangeForNode(sf, result.Node.AsNode()),
						Outer:    result,
						Inner:      inner,
					})
				}
			}
		}

		for child := range n.IterChildren() {
			walk(child)
		}
	}

	walk(sf.AsNode())
	return matches
}

