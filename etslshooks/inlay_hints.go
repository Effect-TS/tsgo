package etslshooks

import (
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/astnav"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	"github.com/microsoft/typescript-go/shim/ls/lsconv"
	"github.com/microsoft/typescript-go/shim/ls/lsutil"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
)

// afterInlayHints filters out redundant return-type inlay hints on Effect.gen,
// Effect.fn, Effect.fnUntraced, and Effect.fnUntracedEager generator functions.
func afterInlayHints(
	program checker.Program,
	c *checker.Checker,
	sf *ast.SourceFile,
	_ core.TextRange,
	preferences *lsutil.InlayHintsPreferences,
	hints []*lsproto.InlayHint,
	converters *lsconv.Converters,
) []*lsproto.InlayHint {
	tp := typeparser.NewTypeParser(program, c)

	effectConfig := program.Options().Effect
	if effectConfig == nil || !effectConfig.Inlays {
		return hints
	}

	if !preferences.IncludeInlayFunctionLikeReturnTypeHints {
		return hints
	}

	result := make([]*lsproto.InlayHint, 0, len(hints))
	for _, hint := range hints {
		if shouldOmitHint(tp, sf, hint, converters) {
			continue
		}
		result = append(result, hint)
	}
	return result
}

// shouldOmitHint checks whether a single inlay hint should be suppressed
// because it is a return-type hint on an Effect generator function.
func shouldOmitHint(
	tp *typeparser.TypeParser,
	sf *ast.SourceFile,
	hint *lsproto.InlayHint,
	converters *lsconv.Converters,
) bool {
	if hint.Kind == nil || *hint.Kind != lsproto.InlayHintKindType {
		return false
	}

	// Convert LSP position back to raw text offset
	offset := int(converters.LineAndCharacterToPosition(sf, hint.Position))

	// Find the token at offset-1 (matching the TS reference: findNodeAtPositionIncludingTrivia(sf, position - 1))
	node := astnav.GetTokenAtPosition(sf, offset-1)
	if node == nil || node.Parent == nil {
		return false
	}

	// Walk up from the token to find the CallExpression.
	// The token at offset-1 is typically the CloseParenToken of function*().
	// Its parent is the FunctionExpression, and grandparent is the CallExpression
	// (e.g. Effect.gen(function*() { ... })). For curried variants like
	// Effect.fn("name")(function*() { ... }), the outer CallExpression may be
	// one more level up. Try ancestors up to a reasonable depth.
	var genNode *ast.FunctionExpression
	var genBody *ast.BlockOrExpression
	for ancestor := node.Parent; ancestor != nil; ancestor = ancestor.Parent {
		if ancestor.Kind == ast.KindCallExpression {
			genNode, genBody = matchEffectGenCall(tp, ancestor)
			if genNode != nil && genBody != nil {
				break
			}
		}
	}
	if genNode == nil || genBody == nil {
		return false
	}

	// Check if the hint position falls between the close paren of the generator
	// function's parameter list and the start of the body
	closeParen := astnav.FindChildOfKind(genNode.AsNode(), ast.KindCloseParenToken, sf)
	if closeParen == nil {
		return false
	}

	bodyStart := astnav.GetStartOfNode(genBody, sf, false)
	return offset >= closeParen.End() && offset <= bodyStart
}

// matchEffectGenCall tries all four Effect generator call patterns and returns
// the generator function and body for the first match.
func matchEffectGenCall(tp *typeparser.TypeParser, node *ast.Node) (*ast.FunctionExpression, *ast.BlockOrExpression) {
	if result := tp.EffectGenCall(node); result != nil {
		return result.GeneratorFunction, result.Body
	}
	if result := tp.EffectFnCall(node); result != nil && result.IsGenerator() {
		return result.GeneratorFunction(), result.Body()
	}
	return nil, nil
}
