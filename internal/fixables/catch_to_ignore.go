package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
)

var CatchToIgnoreFix = fixable.Fixable{
	Name:        "catchToIgnore",
	Description: "Replace Effect.catch/catchCause + Effect.void with Effect.ignore/ignoreCause",
	ErrorCodes:  []int32{tsdiag.Effect_1_expresses_ignored_failure_more_directly_than_Effect_0_returning_Effect_void_effect_catchToIgnore.Code()},
	FixIDs:      []string{"catchToIgnore_fix"},
	Run:         runCatchToIgnoreFix,
}

func runCatchToIgnoreFix(ctx *fixable.Context) []ls.CodeAction {
	sf := ctx.SourceFile

	matches := rules.AnalyzeCatchToIgnore(ctx.TypeParser, ctx.Checker, sf)
	for _, match := range matches {
		diagRange := match.Location
		if !diagRange.Intersects(ctx.Span) && !ctx.Span.ContainedBy(diagRange) {
			continue
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect." + match.IgnoreMethodName,
			Run: func(tracker *rewriter.Tracker) {
				ignoreAccess := catchToIgnoreReplacementAccess(tracker, match)
				if match.DataCallSubject != nil {
					ignoreCall := tracker.NewCallExpression(ignoreAccess, nil, nil, tracker.NewNodeList([]*ast.Node{tracker.DeepCloneNode(match.DataCallSubject)}), ast.NodeFlagsNone)
					ast.SetParentInChildren(ignoreCall)
					tracker.ReplaceNode(sf, match.TransformationNode, ignoreCall, nil)
					return
				}

				tracker.ReplaceNode(sf, match.TransformationNode, ignoreAccess, nil)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}

	return nil
}

func catchToIgnoreReplacementAccess(tracker *rewriter.Tracker, match rules.CatchToIgnoreMatch) *ast.Node {
	var effectModule *ast.Node
	if match.EffectModuleNode != nil {
		effectModule = tracker.DeepCloneNode(match.EffectModuleNode)
	} else {
		effectModule = tracker.NewIdentifier("Effect")
	}
	replacement := tracker.NewPropertyAccessExpression(effectModule, nil, tracker.NewIdentifier(match.IgnoreMethodName), ast.NodeFlagsNone)
	ast.SetParentInChildren(replacement)
	return replacement
}
