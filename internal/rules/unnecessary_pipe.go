// Package rules contains all Effect diagnostic rule implementations.
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

// UnnecessaryPipe detects pipe() and .pipe() calls with no transformation
// arguments and suggests removing the unnecessary pipe wrapper.
var UnnecessaryPipe = rule.Rule{
	Name:            "unnecessaryPipe",
	Group:           "style",
	Description:     "Removes pipe calls with no arguments",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.This_pipe_call_contains_no_arguments_effect_unnecessaryPipe.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeUnnecessaryPipe(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			diags[i] = ctx.NewDiagnostic(m.SourceFile, m.Location, tsdiag.This_pipe_call_contains_no_arguments_effect_unnecessaryPipe, nil)
		}
		return diags
	},
}

// UnnecessaryPipeMatch holds the diagnostic and parsed pipe call result needed
// by both the diagnostic rule and the quick-fix.
type UnnecessaryPipeMatch struct {
	SourceFile *ast.SourceFile                  // The source file where this match was found
	Location   core.TextRange                   // The pre-computed error range for this match
	Result     *typeparser.ParsedPipeCallResult // The parsed pipe call (contains Subject node and call Node)
}

// AnalyzeUnnecessaryPipe finds all pipe() and .pipe() calls with no transformation
// arguments, returning matches with both the diagnostic and the parsed result.
func AnalyzeUnnecessaryPipe(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile) []UnnecessaryPipeMatch {
	var matches []UnnecessaryPipeMatch

	var walk ast.Visitor
	walk = func(n *ast.Node) bool {
		if n == nil {
			return false
		}

		if n.Kind == ast.KindCallExpression {
			if result := tp.ParsePipeCall(n); result != nil {
				if len(result.Args) == 0 {
					matches = append(matches, UnnecessaryPipeMatch{
						SourceFile: sf,
						Location:   scanner.GetErrorRangeForNode(sf, result.Node.AsNode()),
						Result:     result,
					})
				}
			}
		}

		n.ForEachChild(walk)
		return false
	}

	walk(sf.AsNode())
	return matches
}
