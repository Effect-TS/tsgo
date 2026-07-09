package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

var RedundantSchemaTagIdentifier = rule.Rule{
	Name:            "redundantSchemaTagIdentifier",
	Group:           "style",
	Description:     "Suggests removing redundant identifier argument when it equals the tag value in Schema.TaggedClass/TaggedError/TaggedRequest",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.Identifier_0_is_redundant_since_it_equals_the_tag_value_effect_redundantSchemaTagIdentifier.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeRedundantSchemaTagIdentifier(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, 0, len(matches))
		for _, m := range matches {
			if m.KeyStringLiteral.Kind != ast.KindStringLiteral {
				continue
			}
			diags = append(diags, ctx.NewDiagnostic(m.SourceFile, m.Location, tsdiag.Identifier_0_is_redundant_since_it_equals_the_tag_value_effect_redundantSchemaTagIdentifier, nil, m.KeyStringLiteral.AsStringLiteral().Text))
		}
		return diags
	},
}

// RedundantSchemaTagIdentifierMatch holds the AST nodes needed by both the diagnostic rule
// and the quick-fix for the redundantSchemaTagIdentifier pattern.
type RedundantSchemaTagIdentifierMatch struct {
	SourceFile       *ast.SourceFile
	Location         core.TextRange // Pre-computed error range for the diagnostic (on KeyStringLiteral)
	KeyStringLiteral *ast.Node      // The redundant identifier string literal in the inner call
}

// AnalyzeRedundantSchemaTagIdentifier finds all class declarations where the identifier
// string literal equals the tag value in Schema.TaggedClass/TaggedError/TaggedRequest.
func AnalyzeRedundantSchemaTagIdentifier(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []RedundantSchemaTagIdentifierMatch {
	var matches []RedundantSchemaTagIdentifierMatch

	nodeToVisit := make([]*ast.Node, 0)
	pushChild := func(child *ast.Node) bool {
		nodeToVisit = append(nodeToVisit, child)
		return false
	}
	sf.AsNode().ForEachChild(pushChild)

	for len(nodeToVisit) > 0 {
		node := nodeToVisit[len(nodeToVisit)-1]
		nodeToVisit = nodeToVisit[:len(nodeToVisit)-1]

		if node.Kind == ast.KindClassDeclaration {
			if m, ok := analyzeRedundantSchemaTagIdentifierNode(tp, c, sf, node); ok {
				matches = append(matches, m)
			}
		}

		node.ForEachChild(pushChild)
	}

	return matches
}

func analyzeRedundantSchemaTagIdentifierNode(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile, node *ast.Node) (RedundantSchemaTagIdentifierMatch, bool) {
	// Try ExtendsSchemaTaggedClass, then TaggedError, then TaggedRequest (short-circuit on first match)
	result := tp.ExtendsSchemaTaggedClass(node)
	if result == nil {
		result = tp.ExtendsSchemaTaggedError(node)
	}
	if result == nil {
		result = tp.ExtendsSchemaTaggedRequest(node)
	}
	if result == nil {
		return RedundantSchemaTagIdentifierMatch{}, false
	}

	// Both key and tag must be present and must be string literals
	if result.KeyStringLiteral == nil || result.TagStringLiteral == nil {
		return RedundantSchemaTagIdentifierMatch{}, false
	}
	if result.KeyStringLiteral.Kind != ast.KindStringLiteral || result.TagStringLiteral.Kind != ast.KindStringLiteral {
		return RedundantSchemaTagIdentifierMatch{}, false
	}

	keyText := result.KeyStringLiteral.AsStringLiteral().Text
	tagText := result.TagStringLiteral.AsStringLiteral().Text

	if keyText != tagText {
		return RedundantSchemaTagIdentifierMatch{}, false
	}

	return RedundantSchemaTagIdentifierMatch{
		SourceFile:       sf,
		Location:         scanner.GetErrorRangeForNode(sf, result.KeyStringLiteral),
		KeyStringLiteral: result.KeyStringLiteral,
	}, true
}
