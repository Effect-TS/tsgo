package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
)

var ClassSelfMismatchFix = fixable.Fixable{
	Name:        "classSelfMismatch",
	Description: "Replace Self type parameter with the correct class name",
	ErrorCodes:  []int32{tsdiag.The_Self_type_parameter_for_this_class_should_be_0_effect_classSelfMismatch.Code()},
	FixIDs:      []string{"classSelfMismatch_fix"},
	Run:         runClassSelfMismatchFix,
}

func runClassSelfMismatchFix(ctx *fixable.Context) []ls.CodeAction {

	c := ctx.Checker

	sf := ctx.SourceFile

	matches := rules.AnalyzeClassSelfMismatch(ctx.TypeParser, c, sf)
	for _, match := range matches {
		diagRange := match.Location
		if !diagRange.Intersects(ctx.Span) && !ctx.Span.ContainedBy(diagRange) {
			continue
		}

		// Determine the target node to replace. If the Self type node is a TypeReferenceNode,
		// replace only the TypeName identifier (preserving any type arguments).
		// Otherwise, replace the entire Self type node.
		targetNode := match.SelfTypeNode
		if ast.IsTypeReferenceNode(match.SelfTypeNode) {
			targetNode = match.SelfTypeNode.AsTypeReferenceNode().TypeName
		}

		description := "Replace '" + match.ActualName + "' with '" + match.ExpectedName + "'"
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: description,
			Run: func(tracker *change.Tracker) {
				tracker.ReplaceNode(sf, targetNode, tracker.NewIdentifier(match.ExpectedName), nil)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
		return nil
	}

	return nil
}
