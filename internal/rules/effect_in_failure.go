// Package rules contains all Effect diagnostic rule implementations.
package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
)

// EffectInFailure detects when an Effect type appears in the failure (E) channel
// of another Effect. Putting Effect computations in the failure channel is not
// intended; only failure types should appear there.
var EffectInFailure = rule.Rule{
	Name:            "effectInFailure",
	Group:           "antipattern",
	Description:     "Warns when an Effect is used inside an Effect failure channel",
	DefaultSeverity: etscore.SeverityWarning,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.The_error_channel_contains_an_Effect_0_Putting_Effect_computations_in_the_failure_channel_is_not_intended_keep_only_failure_types_there_effect_effectInFailure.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		var diags []*ast.Diagnostic

		// Post-order AST walk using a stack with a visited set.
		// Children are processed before parents so that the
		// shouldSkipBecauseChildMatched mechanism can suppress
		// redundant diagnostics on parent nodes.
		type stackEntry struct {
			node    *ast.Node
			visited bool
		}

		stack := []stackEntry{{node: ctx.SourceFile.AsNode()}}
		shouldSkip := map[*ast.Node]bool{}
		pushChild := func(child *ast.Node) bool {
			stack = append(stack, stackEntry{node: child})
			return false
		}

		for len(stack) > 0 {
			entry := stack[len(stack)-1]
			stack = stack[:len(stack)-1]

			node := entry.node

			// First visit: push self again (marked visited) then push children
			if !entry.visited {
				stack = append(stack, stackEntry{node: node, visited: true})
				node.ForEachChild(pushChild)
				continue
			}

			// Second visit (post-order): check the node

			// If a child already matched, propagate skip to parent and continue
			if shouldSkip[node] {
				if node.Parent != nil {
					shouldSkip[node.Parent] = true
				}
				continue
			}

			nodeType := typeparser.GetTypeAtLocation(ctx.Checker, node)
			if nodeType == nil {
				continue
			}

			effect := typeparser.StrictEffectType(ctx.Checker, nodeType, node)
			if effect == nil {
				continue
			}

			// Check if any union member of the failure channel (E) is a strict Effect type
			failureMembers := typeparser.UnrollUnionMembers(effect.E)
			memberWithEffect := findFirstStrictEffect(ctx.Checker, failureMembers, node)
			if memberWithEffect == nil {
				continue
			}

			diag := ctx.NewDiagnostic(
				ctx.SourceFile,
				ctx.GetErrorRange(node),
				tsdiag.The_error_channel_contains_an_Effect_0_Putting_Effect_computations_in_the_failure_channel_is_not_intended_keep_only_failure_types_there_effect_effectInFailure,
				nil,
				ctx.Checker.TypeToString(memberWithEffect),
			)
			diags = append(diags, diag)

			// Mark parent to skip redundant reporting
			if node.Parent != nil {
				shouldSkip[node.Parent] = true
			}
		}

		return diags
	},
}
