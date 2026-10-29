// Package rules contains all Effect diagnostic rule implementations.
package rules

import (
	"strings"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

// AnyUnknownInErrorContext detects 'any' or 'unknown' types in the error (E) or
// requirements (R) channels of Effect and Layer types.
var AnyUnknownInErrorContext = rule.Rule{
	Name:            "anyUnknownInErrorContext",
	Group:           "correctness",
	Description:     "Detects 'any' or 'unknown' types in Effect error or requirements channels",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.X_0_effect_anyUnknownInErrorContext.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		type matchEntry struct {
			node        *ast.Node
			messageText string
			flagged     anyUnknownChannels
			start       int
			end         int
		}

		var matches []matchEntry

		// Stack-based traversal
		nodeToVisit := make([]*ast.Node, 0)
		pushChild := func(child *ast.Node) bool {
			nodeToVisit = append(nodeToVisit, child)
			return false
		}
		ctx.SourceFile.AsNode().ForEachChild(pushChild)

		for len(nodeToVisit) > 0 {
			// Pop from the end (stack)
			node := nodeToVisit[len(nodeToVisit)-1]
			nodeToVisit = nodeToVisit[:len(nodeToVisit)-1]

			// Skip type nodes
			if ast.IsTypeNode(node) {
				continue
			}
			// Skip type alias declarations
			if node.Kind == ast.KindTypeAliasDeclaration {
				continue
			}
			// Skip interface declarations
			if node.Kind == ast.KindInterfaceDeclaration {
				continue
			}
			// Skip "as any" expressions
			if node.Kind == ast.KindAsExpression {
				if typeNode := node.Type(); typeNode != nil && typeNode.Kind == ast.KindAnyKeyword {
					continue
				}
			}

			// If this is a parameter, property, or variable declaration with explicit
			// Effect or Layer type annotation, skip it entirely (user intentionally typed it)
			if node.Kind == ast.KindParameter || node.Kind == ast.KindPropertyDeclaration || node.Kind == ast.KindVariableDeclaration {
				if typeNode := node.Type(); typeNode != nil {
					if ctx.TypeParser.StrictEffectTypeAtLocation(typeNode) != nil {
						continue
					}
					if ctx.TypeParser.LayerTypeAtLocation(typeNode) != nil {
						continue
					}
				}
			}

			// Enqueue children for visiting
			node.ForEachChild(pushChild)

			if _, ok := typeparser.TypeEquivalentNode(node); ok {
				continue
			}

			// For call expressions, use the resolved signature's return type.
			// Try strict Effect type first, then Layer type
			var sig *checker.Signature
			if node.Kind == ast.KindCallExpression {
				sig = ctx.Checker.GetResolvedSignature(node)
			}
			var eff *typeparser.Effect
			var layer *typeparser.Layer
			if sig != nil {
				t := ctx.Checker.GetReturnTypeOfSignature(sig)
				if eff = ctx.TypeParser.StrictEffectType(t); eff == nil {
					layer = ctx.TypeParser.LayerType(t)
				}
			} else if eff = ctx.TypeParser.StrictEffectTypeAtLocation(node); eff == nil {
				layer = ctx.TypeParser.LayerTypeAtLocation(node)
			}

			var eType, rType *checker.Type
			if eff != nil {
				eType = eff.E()
				rType = eff.R()
			} else if layer != nil {
				eType = layer.E()
				rType = layer.RIn()
			}

			if eType == nil || rType == nil {
				continue
			}

			flagged := anyUnknownChannels{
				r: rType.Flags() & (checker.TypeFlagsAny | checker.TypeFlagsUnknown),
				e: eType.Flags() & (checker.TypeFlagsAny | checker.TypeFlagsUnknown),
			}
			hasAnyUnknownR := flagged.r != 0
			hasAnyUnknownE := flagged.e != 0

			if !hasAnyUnknownR && !hasAnyUnknownE {
				continue
			}

			// Build the channel descriptions
			var channels []string
			if hasAnyUnknownR {
				typeName := "unknown"
				if rType.Flags()&checker.TypeFlagsAny != 0 {
					typeName = "any"
				}
				channels = append(channels, typeName+" in the requirements channel")
			}
			if hasAnyUnknownE {
				typeName := "unknown"
				if eType.Flags()&checker.TypeFlagsAny != 0 {
					typeName = "any"
				}
				channels = append(channels, typeName+" in the error channel")
			}

			// Compose the diagnostic message
			var suggestions []string
			suggestions = append(suggestions, "This has "+strings.Join(channels, " and ")+" which is not recommended.")
			if hasAnyUnknownR {
				suggestions = append(suggestions, "Only service identifiers should appear in the requirements channel.")
			}
			if hasAnyUnknownE {
				suggestions = append(suggestions, "Having an unknown or any error type is not useful. Consider instead using specific error types baked by Data.TaggedError for example.")
			}
			messageText := strings.Join(suggestions, "\n")

			nodeStart := scanner.GetTokenPosOfNode(node, ctx.SourceFile, false)
			nodeEnd := node.End()

			// Innermost-node deduplication: remove parent nodes that contain this
			// node when this node already reports all of their channels
			for i := len(matches) - 1; i >= 0; i-- {
				if matches[i].start <= nodeStart && matches[i].end >= nodeEnd && flagged.covers(matches[i].flagged) {
					matches = append(matches[:i], matches[i+1:]...)
				}
			}

			matches = append(matches, matchEntry{
				node:        node,
				messageText: messageText,
				flagged:     flagged,
				start:       nodeStart,
				end:         nodeEnd,
			})
		}

		// Report all innermost matching nodes
		var diags []*ast.Diagnostic
		for _, m := range matches {
			diags = append(diags, ctx.NewDiagnostic(ast.GetSourceFileOfNode(m.node), ctx.GetErrorRange(m.node), tsdiag.X_0_effect_anyUnknownInErrorContext, nil, m.messageText))
		}

		return diags
	},
}

// anyUnknownChannels holds TypeFlagsAny or TypeFlagsUnknown for each channel
// that has one, and zero for a channel that has neither.
type anyUnknownChannels struct {
	r checker.TypeFlags
	e checker.TypeFlags
}

// covers reports whether c flags every channel that other flags, with the
// same any or unknown.
func (c anyUnknownChannels) covers(other anyUnknownChannels) bool {
	return (other.r == 0 || other.r == c.r) && (other.e == 0 || other.e == c.e)
}
