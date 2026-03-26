package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
)

var DeterministicKeysFix = fixable.Fixable{
	Name:        "deterministicKeys",
	Description: "Replace key with expected deterministic key",
	ErrorCodes:  []int32{tsdiag.Key_should_be_0_effect_deterministicKeys.Code()},
	FixIDs:      []string{"deterministicKeys_fix"},
	Run:         runDeterministicKeysFix,
}

func runDeterministicKeysFix(ctx *fixable.Context) []ls.CodeAction {
	c, done := ctx.GetTypeCheckerForFile(ctx.SourceFile)
	if c == nil {
		return nil
	}
	defer done()

	matches := rules.AnalyzeDeterministicKeys(c, ctx.SourceFile)
	for _, match := range matches {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}

		// Determine the quote style from the original string literal
		var flags ast.TokenFlags
		if match.KeyStringLiteral.AsStringLiteral().TokenFlags&ast.TokenFlagsSingleQuote != 0 {
			flags = ast.TokenFlagsSingleQuote
		}

		description := "Replace '" + match.ActualKey + "' with '" + match.ExpectedKey + "'"
		sf := ctx.SourceFile
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: description,
			Run: func(tracker *change.Tracker) {
				tracker.ReplaceNode(sf, match.KeyStringLiteral, tracker.NewStringLiteral(match.ExpectedKey, flags), nil)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
		return nil
	}

	return nil
}
