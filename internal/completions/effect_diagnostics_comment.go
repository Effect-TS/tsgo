package completions

import (
	"fmt"
	"regexp"
	"sort"
	"strings"

	"github.com/effect-ts/tsgo/internal/completion"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
)

// effectDiagnosticsComment provides completions for @effect-diagnostics and
// @effect-diagnostics-next-line directives when typing @ inside a comment.
var effectDiagnosticsComment = completion.Completion{
	Name:        "effectDiagnosticsComment",
	Description: "Provides @effect-diagnostics completion inside comments",
	Run:         runEffectDiagnosticsComment,
}

// commentAtRegex matches `//`, `/*`, or `/**` followed by optional whitespace and `@` at end of string.
var commentAtRegex = regexp.MustCompile(`(//|/\*\*?)\s*(@)\s*$`)

func runEffectDiagnosticsComment(ctx *completion.Context) []*lsproto.CompletionItem {
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

	// Collect and sort all diagnostic rule names
	names := make([]string, len(rules.All))
	for i, r := range rules.All {
		names[i] = r.Name
	}
	sort.Strings(names)

	allDiagnostics := strings.Join(names, ",")
	disableSnippet := fmt.Sprintf("${1|%s|}:${2|off,warning,error,message,suggestion|}$0", allDiagnostics)

	sortText := "11"

	return []*lsproto.CompletionItem{
		makeCompletionItem(
			"@effect-diagnostics",
			"@effect-diagnostics "+disableSnippet,
			sortText,
			replacementRange,
			lsproto.CompletionItemKindConstant,
		),
		makeCompletionItem(
			"@effect-diagnostics-next-line",
			"@effect-diagnostics-next-line "+disableSnippet,
			sortText,
			replacementRange,
			lsproto.CompletionItemKindConstant,
		),
	}
}
