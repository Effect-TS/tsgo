package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/effect-ts/tsgo/internal/rewriter"
)

var DeterministicKeysFix = fixable.Fixable{
	Name:        "deterministicKeys",
	Description: "Replace key with expected deterministic key",
	ErrorCodes:  []int32{tsdiag.This_key_does_not_match_the_deterministic_key_for_this_declaration_The_expected_key_is_0_effect_deterministicKeys.Code()},
	FixIDs:      []string{"deterministicKeys_fix"},
	Run:         runDeterministicKeysFix,
}

func runDeterministicKeysFix(ctx *fixable.Context) []ls.CodeAction {
	matches := rules.AnalyzeDeterministicKeys(ctx.TypeParser, ctx.Program, ctx.Checker, ctx.SourceFile, ctx.Options)
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
			Run: func(tracker *rewriter.Tracker) {
				tracker.ReplaceNode(sf, match.KeyStringLiteral, tracker.NewStringLiteral(match.ExpectedKey, flags), nil)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
		return nil
	}

	return nil
}
