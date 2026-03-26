package fixables

import (
	"fmt"

	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
	"github.com/microsoft/typescript-go/shim/scanner"
)

var RedundantSchemaTagIdentifierRemoveIdentifierFix = fixable.Fixable{
	Name:        "redundantSchemaTagIdentifier",
	Description: "Remove redundant identifier",
	ErrorCodes:  []int32{tsdiag.Identifier_0_is_redundant_since_it_equals_the_tag_value_effect_redundantSchemaTagIdentifier.Code()},
	FixIDs:      []string{"redundantSchemaTagIdentifier_removeIdentifier"},
	Run:         runRedundantSchemaTagIdentifierFix,
}

func runRedundantSchemaTagIdentifierFix(ctx *fixable.Context) []ls.CodeAction {
	c, done := ctx.GetTypeCheckerForFile(ctx.SourceFile)
	if c == nil {
		return nil
	}
	defer done()

	sf := ctx.SourceFile

	matches := rules.AnalyzeRedundantSchemaTagIdentifier(c, sf)
	for _, match := range matches {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}

		keyText := match.KeyStringLiteral.AsStringLiteral().Text
		keyNode := match.KeyStringLiteral

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: fmt.Sprintf("Remove redundant identifier '%s'", keyText),
			Run: func(tracker *change.Tracker) {
				tokenPos := scanner.GetTokenPosOfNode(keyNode, sf, false)
				tracker.DeleteRange(sf, core.NewTextRange(tokenPos, keyNode.End()))
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
		return nil
	}

	return nil
}
