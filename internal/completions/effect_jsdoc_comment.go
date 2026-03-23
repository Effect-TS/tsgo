package completions

import (
	"github.com/effect-ts/effect-typescript-go/internal/completion"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
)

// effectJsdocComment provides completion for @effect-identifier
// when typing @ inside a comment.
var effectJsdocComment = completion.Completion{
	Name:        "effectJsdocComment",
	Description: "Provides @effect-identifier completion inside comments",
	Run:         runEffectJsdocComment,
}

func runEffectJsdocComment(ctx *completion.Context) []*lsproto.CompletionItem {
	sourceText := ctx.SourceFile.Text()
	textBeforeCursor := sourceText[:ctx.Position]

	loc := commentAtRegex.FindStringSubmatchIndex(textBeforeCursor)
	if loc == nil {
		return nil
	}

	// loc[4] and loc[5] are the start/end byte offsets of the `@` capture group
	atStart := loc[4]
	spanLength := max(ctx.Position-atStart, 0)

	replacementRange := byteSpanToRange(ctx, atStart, spanLength)

	sortText := "11"

	return []*lsproto.CompletionItem{
		makeCompletionItem(
			"@effect-identifier",
			"@effect-identifier",
			sortText,
			replacementRange,
			lsproto.CompletionItemKindConstant,
		),
	}
}
