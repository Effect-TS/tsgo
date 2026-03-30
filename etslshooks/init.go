// Package etslshooks provides Effect code fix integration with TypeScript-Go.
// This package registers a single CodeFixProvider that delegates to internal/fixables.
//
// Import this package with a blank import in cmd/tsgo/main.go to register
// Effect code fix providers:
//
//	import _ "github.com/effect-ts/tsgo/etslshooks"
package etslshooks

import (
	"context"
	"fmt"
	"strings"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/autoimportstyle"
	"github.com/effect-ts/tsgo/internal/completion"
	"github.com/effect-ts/tsgo/internal/completions"
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/fixables"
	"github.com/effect-ts/tsgo/internal/layergraph"
	"github.com/effect-ts/tsgo/internal/pluginoptions"
	"github.com/effect-ts/tsgo/internal/refactor"
	"github.com/effect-ts/tsgo/internal/refactors"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/compiler"
	"github.com/microsoft/typescript-go/shim/core"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/autoimport"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
	"github.com/microsoft/typescript-go/shim/modulespecifiers"
)

func init() {
	// Register the Effect code fix provider with the language service
	ls.RegisterCodeFixProvider(effectFixProvider)
	// Register the Effect refactor provider with the language service
	ls.RegisterRefactorProvider(effectRefactorProvider)
	// Register the Effect hover enrichment callback
	ls.RegisterAfterQuickInfoCallback(afterQuickInfo)
	// Register the Effect document symbol enrichment callback
	ls.RegisterAfterDocumentSymbolsCallback(afterDocumentSymbols)
	// Register the Effect inlay hints suppression callback
	ls.RegisterAfterInlayHintsCallback(afterInlayHints)
	// Register the Effect completion enrichment callback
	ls.RegisterAfterCompletionCallback(afterCompletion)
	// Register the Effect auto-import style transformer factory
	autoimport.RegisterAutoImportFixTransformer(func(_ modulespecifiers.UserPreferences, program *compiler.Program, importingFile *ast.SourceFile) autoimport.FixTransformer {
		var resolvedOptions *etscore.ResolvedEffectPluginOptions
		if effectConfig := program.Options().Effect; effectConfig != nil {
			resolvedOptions = pluginoptions.ResolveEffectPluginOptionsForSourceFile(
				effectConfig,
				importingFile.FileName(),
				program.Options().ConfigFilePath,
				program.UseCaseSensitiveFileNames(),
			)
		}
		return autoimportstyle.NewFixTransformer(resolvedOptions)
	})
}

// effectFixProvider is the CodeFixProvider that handles all Effect diagnostic codes.
// It delegates to the fixables registered in internal/fixables.
var effectFixProvider = &ls.CodeFixProvider{
	ErrorCodes:     fixables.AllErrorCodes(),
	GetCodeActions: getEffectCodeActions,
	FixIds:         fixables.AllFixIDs(),
}

// getEffectCodeActions finds applicable fixables and collects their code actions.
func getEffectCodeActions(ctx context.Context, fixCtx *ls.CodeFixContext) ([]ls.CodeAction, error) {
	// Find all fixables that handle this error code
	applicable := fixables.ByErrorCode(fixCtx.ErrorCode)
	if len(applicable) == 0 {
		return nil, nil
	}

	var options *etscore.ResolvedEffectPluginOptions
	if fixCtx.Program != nil {
		if parsedEffectConfig := fixCtx.Program.Options().Effect; parsedEffectConfig != nil {
			options = pluginoptions.ResolveEffectPluginOptionsForSourceFile(
				parsedEffectConfig,
				fixCtx.SourceFile.FileName(),
				fixCtx.Program.Options().ConfigFilePath,
				fixCtx.Program.UseCaseSensitiveFileNames(),
			)
		}
	}

	// Create the fixable context that wraps the code-fix request
	fCtx := fixable.NewContext(ctx, fixCtx, options)

	// Collect actions from all applicable fixables
	var actions []ls.CodeAction
	for _, f := range applicable {
		results := f.Run(fCtx)
		actions = append(actions, results...)
	}

	return actions, nil
}

// effectRefactorProvider is the RefactorProvider that handles all Effect refactoring actions.
// It delegates to the refactors registered in internal/refactors.
var effectRefactorProvider = &ls.RefactorProvider{
	GetRefactorActions: getEffectRefactorActions,
}

// getEffectRefactorActions iterates all registered refactors and collects their code actions.
func getEffectRefactorActions(ctx context.Context, file *ast.SourceFile, span core.TextRange, program *compiler.Program, langService *ls.LanguageService) ([]ls.CodeAction, error) {
	if effectConfig := program.Options().Effect; effectConfig == nil || !effectConfig.GetRefactorsEnabled() {
		return nil, nil
	}

	rCtx := refactor.NewContext(ctx, file, span, program, langService)

	var actions []ls.CodeAction
	for _, r := range refactors.All {
		results := r.Run(rCtx)
		actions = append(actions, results...)
	}

	return actions, nil
}

// afterCompletion is called after TypeScript-Go builds the completion list.
// It allows Effect to enrich completion responses with custom completions.
func afterCompletion(ctx context.Context, sf *ast.SourceFile, position int, items []*lsproto.CompletionItem, program *compiler.Program, langService *ls.LanguageService) []*lsproto.CompletionItem {
	effectConfig := program.Options().Effect
	if effectConfig == nil || !effectConfig.GetCompletionsEnabled() {
		return items
	}

	if len(completions.All) == 0 {
		return items
	}

	completionCtx := completion.NewContext(ctx, sf, position, items, program, langService)

	for _, c := range completions.All {
		results := c.Run(completionCtx)
		items = append(items, results...)
	}

	return items
}

// afterQuickInfo is called after building hover quickInfo and documentation.
// It allows Effect to enrich hover responses with Effect-specific information.
func afterQuickInfo(c *checker.Checker, sf *ast.SourceFile, node *ast.Node, _ *ast.Symbol, quickInfo string, documentation string, isMarkdown bool) (string, string, *ast.Node) {
	// Check if Effect is enabled
	effectConfig := c.Program().Options().Effect
	if effectConfig == nil || !effectConfig.GetQuickinfoEnabled() {
		return quickInfo, documentation, nil
	}

	// Yield* hover: detect yield keyword inside yield* expressions in Effect generator scopes
	if node.Kind == ast.KindYieldKeyword && node.Parent != nil && node.Parent.Kind == ast.KindYieldExpression {
		yield := node.Parent.AsYieldExpression()
		if yield.AsteriskToken != nil && yield.Expression != nil {
			if typeparser.GetEffectContextFlags(c, node)&typeparser.EffectContextFlagCanYieldEffect != 0 {
				t := typeparser.GetTypeAtLocation(c, yield.Expression)
				if t != nil {
					effect := typeparser.EffectYieldableType(c, t, yield.Expression)
					if effect != nil {
						typeStr := c.TypeToStringEx(t, nil, checker.TypeFormatFlagsNoTruncation)
						quickInfo = "(yield*) " + typeStr
						documentation = formatEffectTypeParams(c, effect, "", isMarkdown)
						return quickInfo, documentation, node.Parent
					}
				}
			}
		}
	}

	// General symbol hover: enrich Effect-typed symbols with type parameters
	t := typeparser.GetTypeAtLocation(c, node)
	if t == nil {
		return quickInfo, documentation, nil
	}

	// Layer hover: detect Layer types and show providers/requirers summary.
	// Layer extends Effect in V4, so this check must come before the Effect check.
	// Only activate layer hover enrichment when the cursor is on the name of the declaration,
	// not on arbitrary nodes within the initializer expression.
	if typeparser.IsLayerType(c, t, node) && isDeclarationName(node) {
		documentation = formatLayerHover(c, sf, node, t, documentation, isMarkdown, effectConfig)
		return quickInfo, documentation, nil
	}

	effect := typeparser.EffectType(c, t, node)
	if effect == nil {
		return quickInfo, documentation, nil
	}

	documentation = formatEffectTypeParams(c, effect, documentation, isMarkdown)

	return quickInfo, documentation, nil
}

// formatLayerHover builds the Layer hover documentation including providers/requirers
// summary, Mermaid diagram links, and Layer type parameters.
func formatLayerHover(c *checker.Checker, sf *ast.SourceFile, node *ast.Node, _ *checker.Type, documentation string, isMarkdown bool, effectConfig *etscore.EffectPluginOptions) string {
	// Try to resolve the initializer expression for layer graph extraction.
	var initializer *ast.Node
	if node.Parent != nil {
		switch node.Parent.Kind {
		case ast.KindVariableDeclaration:
			initializer = node.Parent.AsVariableDeclaration().Initializer
		case ast.KindPropertyDeclaration:
			initializer = node.Parent.AsPropertyDeclaration().Initializer
		}
	}

	var quickInfoSummary string
	var hasGraph bool
	var nestedDiagram, outlineDiagram string
	if initializer != nil {
		opts := layergraph.ExtractLayerGraphOptions{
			FollowSymbolsDepth: effectConfig.GetLayerGraphFollowDepth(),
		}
		fullGraph := layergraph.ExtractLayerGraph(c, initializer, sf, opts)
		info := layergraph.ExtractProvidersAndRequirers(c, fullGraph)
		quickInfoSummary = layergraph.FormatQuickInfo(c, info, sf)
		hasGraph = true

		if !effectConfig.NoExternal {
			nestedDiagram = layergraph.FormatNestedLayerGraph(c, fullGraph, sf)
			outlineGraph := layergraph.ExtractOutlineGraph(c, fullGraph)
			outlineDiagram = layergraph.FormatOutlineGraph(c, outlineGraph, sf)
		}
	}

	// Build combined documentation: quickinfo summary (provides/requires) and links.
	var b strings.Builder

	if quickInfoSummary != "" {
		if isMarkdown {
			b.WriteString("```\n")
			b.WriteString(quickInfoSummary)
			b.WriteString("\n```\n")
		} else {
			b.WriteString(quickInfoSummary)
			b.WriteString("\n")
		}
	}

	// Generate Mermaid diagram links when we have a graph and external links are not suppressed.
	if hasGraph && !effectConfig.NoExternal {
		baseURL := effectConfig.GetMermaidBaseURL()

		var nestedURL, outlineURL string
		if nestedDiagram != "" {
			nestedURL = layergraph.EncodeMermaidURL(baseURL, nestedDiagram)
		}
		if outlineDiagram != "" {
			outlineURL = layergraph.EncodeMermaidURL(baseURL, outlineDiagram)
		}

		if isMarkdown {
			switch {
			case nestedURL != "" && outlineURL != "":
				fmt.Fprintf(&b, "[Show full graph](%s) - [Show outline](%s)\n\n", nestedURL, outlineURL)
			case nestedURL != "":
				fmt.Fprintf(&b, "[Show full graph](%s)\n\n", nestedURL)
			case outlineURL != "":
				fmt.Fprintf(&b, "[Show outline](%s)\n\n", outlineURL)
			}
		} else {
			if nestedURL != "" {
				fmt.Fprintf(&b, "{@link %s Show full graph}\n\n", nestedURL)
			}
			if outlineURL != "" {
				fmt.Fprintf(&b, "{@link %s Show outline}\n\n", outlineURL)
			}
		}
	}

	if documentation != "" {
		b.WriteString("\n")
		b.WriteString(documentation)
	}

	return b.String()
}

// formatLayerTypeParams formats Layer type parameters (Provides, Error, Requires).
func formatLayerTypeParams(c *checker.Checker, layer *typeparser.Layer, isMarkdown bool) string {
	rOutStr := c.TypeToStringEx(layer.ROut, nil, checker.TypeFormatFlagsNoTruncation)
	eStr := c.TypeToStringEx(layer.E, nil, checker.TypeFormatFlagsNoTruncation)
	rInStr := c.TypeToStringEx(layer.RIn, nil, checker.TypeFormatFlagsNoTruncation)

	if isMarkdown {
		return fmt.Sprintf("```ts\n/* Layer Type Parameters */\ntype Provides = %s\ntype Error = %s\ntype Requires = %s\n```\n", rOutStr, eStr, rInStr)
	}
	return fmt.Sprintf("Layer Type Parameters:\n  Provides = %s\n  Error = %s\n  Requires = %s\n", rOutStr, eStr, rInStr)
}

// isDeclarationName checks whether the given node is the name node of a variable or property declaration.
// This is used to restrict layer hover enrichment to the declaration name only,
// not to arbitrary nodes within the initializer expression.
func isDeclarationName(node *ast.Node) bool {
	if node.Parent == nil {
		return false
	}
	switch node.Parent.Kind {
	case ast.KindVariableDeclaration:
		return node.Parent.AsVariableDeclaration().Name() == node
	case ast.KindPropertyDeclaration:
		return node.Parent.AsPropertyDeclaration().Name() == node
	}
	return false
}

// formatEffectTypeParams formats Effect type parameters (A, E, R) and prepends them to documentation.
func formatEffectTypeParams(c *checker.Checker, effect *typeparser.Effect, documentation string, isMarkdown bool) string {
	aStr := c.TypeToStringEx(effect.A, nil, checker.TypeFormatFlagsNoTruncation)
	eStr := c.TypeToStringEx(effect.E, nil, checker.TypeFormatFlagsNoTruncation)
	rStr := c.TypeToStringEx(effect.R, nil, checker.TypeFormatFlagsNoTruncation)

	var prefix string
	if isMarkdown {
		prefix = fmt.Sprintf("```ts\n/* Effect Type Parameters */\ntype Success = %s\ntype Failure = %s\ntype Requirements = %s\n```\n", aStr, eStr, rStr)
	} else {
		prefix = fmt.Sprintf("Effect Type Parameters:\n  Success = %s\n  Failure = %s\n  Requirements = %s\n", aStr, eStr, rStr)
	}

	var b strings.Builder
	b.WriteString(prefix)
	if documentation != "" {
		b.WriteString("\n")
		b.WriteString(documentation)
	}
	return b.String()
}
