package completions

import (
	"fmt"
	"sort"
	"strings"

	"github.com/effect-ts/effect-typescript-go/internal/codegens"
	"github.com/effect-ts/effect-typescript-go/internal/completion"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
)

// effectCodegensComment provides completions for @effect-codegens directives
// when typing @ inside a comment.
var effectCodegensComment = completion.Completion{
	Name:        "effectCodegensComment",
	Description: "Provides @effect-codegens completion inside comments",
	Run:         runEffectCodegensComment,
}

func runEffectCodegensComment(ctx *completion.Context) []*lsproto.CompletionItem {
	sourceText := ctx.SourceFile.Text()
	textBeforeCursor := sourceText[:ctx.Position]

	loc := commentAtRegex.FindStringSubmatchIndex(textBeforeCursor)
	if loc == nil {
		return nil
	}

	// loc[4] and loc[5] are the start/end byte offsets of the `@` capture group
	atStart := loc[4]
	spanLength := ctx.Position - atStart
	if spanLength < 0 {
		spanLength = 0
	}

	replacementRange := byteSpanToRange(ctx, atStart, spanLength)

	// Collect and sort all codegen names
	names := make([]string, len(codegens.All))
	for i, c := range codegens.All {
		names[i] = c.Name
	}
	sort.Strings(names)

	allCodegens := strings.Join(names, ",")
	enableSnippet := fmt.Sprintf("${1|%s|} $0", allCodegens)

	sortText := "11"

	return []*lsproto.CompletionItem{
		makeCompletionItem(
			"@effect-codegens",
			"@effect-codegens "+enableSnippet,
			sortText,
			replacementRange,
		),
	}
}
