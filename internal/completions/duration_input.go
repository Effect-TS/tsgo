package completions

import (
	"strings"

	"github.com/effect-ts/effect-typescript-go/internal/completion"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/astnav"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// durationInput provides completion items for Duration string literals.
// When the cursor is inside a string literal whose contextual type is a Duration union
// (containing template literals with "nanos" text), it offers snippet completions
// for all 8 duration units.
var durationInput = completion.Completion{
	Name:        "durationInput",
	Description: "Provides duration unit completions inside Duration string literals",
	Run:         runDurationInput,
}

var durationUnits = []string{"nanos", "micros", "millis", "seconds", "minutes", "hours", "days", "weeks"}

func runDurationInput(ctx *completion.Context) []*lsproto.CompletionItem {
	previousToken := astnav.FindPrecedingToken(ctx.SourceFile, ctx.Position)
	if previousToken == nil || !ast.IsStringTextContainingNode(previousToken) {
		return nil
	}

	start := scanner.GetTokenPosOfNode(previousToken, ctx.SourceFile, false)
	end := previousToken.End()

	// The cursor must be strictly inside the string literal content,
	// or at the end position of an unterminated literal.
	isInString := start < ctx.Position && ctx.Position < end
	if ctx.Position == end {
		isInString = ast.IsUnterminatedLiteral(previousToken)
	}

	if !isInString || !ast.IsExpression(previousToken) {
		return nil
	}

	ch, done := ctx.GetTypeCheckerForFile(ctx.SourceFile)
	defer done()

	contextualType := ch.GetContextualType(previousToken, checker.ContextFlagsNone)
	if contextualType == nil || !contextualType.IsUnion() {
		return nil
	}

	for _, member := range contextualType.AsUnionType().Types() {
		if member.Flags()&checker.TypeFlagsTemplateLiteral == 0 {
			continue
		}
		texts := member.AsTemplateLiteralType().Texts()
		if len(texts) == 2 && strings.TrimSpace(texts[1]) == "nanos" {
			return buildDurationItems(ctx, previousToken)
		}
	}

	return nil
}

func buildDurationItems(ctx *completion.Context, node *ast.Node) []*lsproto.CompletionItem {
	// Replacement range covers the string content (between the quotes)
	start := scanner.GetTokenPosOfNode(node, ctx.SourceFile, false) + 1 // after opening quote
	end := node.End() - 1                                               // before closing quote
	end = max(end, start)
	replacementRange := byteSpanToRange(ctx, start, end-start)
	sortText := "11"

	items := make([]*lsproto.CompletionItem, len(durationUnits))
	for i, unit := range durationUnits {
		items[i] = makeCompletionItem(
			unit,
			"${0} "+unit,
			sortText,
			replacementRange,
			lsproto.CompletionItemKindConstant,
		)
	}
	return items
}
