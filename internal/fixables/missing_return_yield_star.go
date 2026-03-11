// Package fixables contains all code fix implementations.
package fixables

import (
	"github.com/effect-ts/effect-typescript-go/internal/fixable"
	"github.com/effect-ts/effect-typescript-go/internal/rules"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
)

// MissingReturnYieldStarFix adds "return" before a yield* expression in Effect.gen when needed.
var MissingReturnYieldStarFix = fixable.Fixable{
	Name:        "missingReturnYieldStar",
	Description: "Add return before yield* for never-success Effect yields",
	ErrorCodes:  []int32{tsdiag.It_is_recommended_to_use_return_yield_Asterisk_for_Effects_that_never_succeed_to_signal_a_definitive_exit_point_for_type_narrowing_and_tooling_support_effect_missingReturnYieldStar.Code()},
	FixIDs:      []string{"missingReturnYieldStar_fix"},
	Run:         runMissingReturnYieldStarFix,
}

func runMissingReturnYieldStarFix(ctx *fixable.Context) []ls.CodeAction {

	c, done := ctx.GetTypeCheckerForFile(ctx.SourceFile)
	if c == nil {
		return nil
	}
	defer done()

	sf := ctx.SourceFile

	matches := rules.AnalyzeMissingReturnYieldStar(c, sf)

	var matchedExprStmt *ast.Node
	for _, match := range matches {
		diagRange := match.Location
		if diagRange.Intersects(ctx.Span) || ctx.Span.ContainedBy(diagRange) {
			matchedExprStmt = match.ExprStmtNode
			break
		}
	}
	if matchedExprStmt == nil {
		return nil
	}

	if action := ctx.NewFixAction(fixable.FixAction{
		Description: "Add return statement",
		Run: func(tracker *change.Tracker) {
			clonedExpr := tracker.DeepCloneNode(matchedExprStmt.Expression())
			returnStmt := tracker.NewReturnStatement(clonedExpr)
			ast.SetParentInChildren(returnStmt)
			tracker.ReplaceNode(sf, matchedExprStmt, returnStmt, nil)
		},
	}); action != nil {
		return []ls.CodeAction{*action}
	}
	return nil
}
