package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/core"
)

type EffectContextFlags uint8

const (
	EffectContextFlagNone           EffectContextFlags = 0
	EffectContextFlagCanYieldEffect EffectContextFlags = 1 << iota
	EffectContextFlagInEffectConstructorThunk
	EffectContextFlagPendingNextFunctionIsEffectThunk
	EffectContextFlagPendingNextObjectTryPropertyIsEffectThunk
	EffectContextFlagInEffect = EffectContextFlagCanYieldEffect | EffectContextFlagInEffectConstructorThunk
)

func (tp *TypeParser) GetEffectContextFlags(node *ast.Node) EffectContextFlags {
	if tp == nil {
		return EffectContextFlagNone
	}
	links := tp.ensureEffectContextAnalyzed(node)
	if links == nil {
		return EffectContextFlagNone
	}

	if closest, ok := getClosestNodeWithLinks(&links.EffectContextFlags, node); ok {
		return *links.EffectContextFlags.TryGet(closest) & EffectContextFlagInEffect
	}
	return EffectContextFlagNone
}

func (tp *TypeParser) GetEffectYieldGeneratorFunction(node *ast.Node) *ast.FunctionExpression {
	if tp == nil {
		return nil
	}
	links := tp.ensureEffectContextAnalyzed(node)
	if links == nil {
		return nil
	}

	if closest, ok := getClosestNodeWithLinks(&links.EffectYieldGeneratorFunction, node); ok {
		return *links.EffectYieldGeneratorFunction.TryGet(closest)
	}
	return nil
}

func getClosestNodeWithLinks[T any](store *core.LinkStore[*ast.Node, T], node *ast.Node) (*ast.Node, bool) {
	if store == nil || node == nil {
		return nil, false
	}

	for current := node; current != nil; current = current.Parent {
		if store.Has(current) {
			return current, true
		}
	}

	return nil, false
}

func (tp *TypeParser) ensureEffectContextAnalyzed(node *ast.Node) *EffectLinks {
	if tp == nil || tp.checker == nil || node == nil {
		return nil
	}
	links := tp.links

	if links.EffectContextFlags.Has(node) {
		return links
	}

	sf := ast.GetSourceFileOfNode(node)
	if sf == nil {
		return nil
	}

	Cached(&links.EffectContextAnalyzed, sf, func() bool {
		tp.analyzeEffectContextForSourceFile(sf)
		return true
	})
	return links
}

func (tp *TypeParser) analyzeEffectContextForSourceFile(sf *ast.SourceFile) {
	if tp == nil || tp.checker == nil || sf == nil {
		return
	}
	links := tp.links

	var walk ast.Visitor
	var pendingEnableFlags core.LinkStore[*ast.Node, EffectContextFlags]
	var pendingDisableFlags core.LinkStore[*ast.Node, EffectContextFlags]
	pendingFlagsMask := EffectContextFlagPendingNextFunctionIsEffectThunk | EffectContextFlagPendingNextObjectTryPropertyIsEffectThunk
	functionScopeResetFlags := EffectContextFlagCanYieldEffect | EffectContextFlagInEffectConstructorThunk | pendingFlagsMask

	setPendingEnableFlags := func(node *ast.Node, flags EffectContextFlags) {
		if node == nil || flags == EffectContextFlagNone {
			return
		}
		*pendingEnableFlags.Get(node) |= flags
	}

	setPendingDisableFlags := func(node *ast.Node, flags EffectContextFlags) {
		if node == nil || flags == EffectContextFlagNone {
			return
		}
		*pendingDisableFlags.Get(node) |= flags
	}

	transparentPendingExpression := func(node *ast.Node) *ast.Node {
		if node == nil {
			return nil
		}
		switch node.Kind {
		case ast.KindParenthesizedExpression, ast.KindSatisfiesExpression, ast.KindAsExpression, ast.KindNonNullExpression, ast.KindTypeAssertionExpression:
			return node.Expression()
		default:
			return nil
		}
	}

	resetChildFunctionScopeFlags := func(node *ast.Node) bool {
		setPendingDisableFlags(node, functionScopeResetFlags)
		return false
	}

	resetPendingFlags := func(child *ast.Node) bool {
		setPendingDisableFlags(child, pendingFlagsMask)
		return false
	}

	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}

		if node.Parent != nil {
			// inherit from parent, if any
			*links.EffectContextFlags.Get(node) = *links.EffectContextFlags.Get(node.Parent)
			if !links.EffectYieldGeneratorFunction.Has(node) {
				*links.EffectYieldGeneratorFunction.Get(node) = *links.EffectYieldGeneratorFunction.Get(node.Parent)
			}
		} else {
			// default, no flags.
			*links.EffectContextFlags.Get(node) = EffectContextFlagNone
		}

		// disable pending disable flags
		if pendingDisableFlags.Has(node) {
			*links.EffectContextFlags.Get(node) &^= *pendingDisableFlags.TryGet(node)
		}

		// merge pending state for this node
		if pendingEnableFlags.Has(node) {
			*links.EffectContextFlags.Get(node) |= *pendingEnableFlags.TryGet(node)
		}

		if *links.EffectContextFlags.Get(node)&EffectContextFlagPendingNextFunctionIsEffectThunk != 0 && (node.Kind == ast.KindArrowFunction || node.Kind == ast.KindFunctionExpression) {
			if body := node.Body(); body != nil {
				setPendingEnableFlags(body.AsNode(), EffectContextFlagInEffectConstructorThunk)
				setPendingDisableFlags(body.AsNode(), pendingFlagsMask)
			}
		} else if *links.EffectContextFlags.Get(node)&EffectContextFlagPendingNextObjectTryPropertyIsEffectThunk != 0 && node.Kind == ast.KindObjectLiteralExpression {
			node.ForEachChild(resetPendingFlags)

			obj := node.AsObjectLiteralExpression()
			if obj != nil && obj.Properties != nil {
				for _, prop := range obj.Properties.Nodes {
					if prop == nil || prop.Kind != ast.KindPropertyAssignment {
						continue
					}
					assignment := prop.AsPropertyAssignment()
					if assignment == nil || assignment.Name() == nil || assignment.Initializer == nil {
						continue
					}
					if assignment.Name().Text() != "try" {
						continue
					}
					setPendingEnableFlags(assignment.Initializer, EffectContextFlagPendingNextFunctionIsEffectThunk)
				}
			}
		} else if expr := transparentPendingExpression(node); expr != nil {
			setPendingEnableFlags(expr, *links.EffectContextFlags.Get(node)&pendingFlagsMask)
		} else if *links.EffectContextFlags.Get(node)&pendingFlagsMask != 0 {
			node.ForEachChild(resetPendingFlags)
		}

		// logic for this node
		if effectGen := tp.EffectGenCall(node); effectGen != nil {
			bodyNode := effectGen.Body.AsNode()
			setPendingEnableFlags(bodyNode, EffectContextFlagCanYieldEffect)
			*links.EffectYieldGeneratorFunction.Get(bodyNode) = effectGen.GeneratorFunction
		} else if effectFn := tp.EffectFnCall(node); effectFn != nil && effectFn.IsGenerator() {
			body := effectFn.Body()
			genFn := effectFn.GeneratorFunction()
			if body != nil && genFn != nil {
				bodyNode := body.AsNode()
				setPendingEnableFlags(bodyNode, EffectContextFlagCanYieldEffect)
				*links.EffectYieldGeneratorFunction.Get(bodyNode) = genFn
			}
		}

		if node.Kind == ast.KindCallExpression {
			call := node.AsCallExpression()
			if call != nil && call.Arguments != nil && len(call.Arguments.Nodes) > 0 {
				effectThunkArg := call.Arguments.Nodes[0]
				switch {
				case tp.IsNodeReferenceToEffectModuleApi(call.Expression, "sync"),
					tp.IsNodeReferenceToEffectModuleApi(call.Expression, "promise"),
					tp.IsNodeReferenceToEffectModuleApi(call.Expression, "callback"),
					tp.IsNodeReferenceToEffectModuleApi(call.Expression, "suspend"):
					setPendingEnableFlags(effectThunkArg, EffectContextFlagPendingNextFunctionIsEffectThunk)
				case tp.IsNodeReferenceToEffectModuleApi(call.Expression, "try"),
					tp.IsNodeReferenceToEffectModuleApi(call.Expression, "tryPromise"):
					setPendingEnableFlags(effectThunkArg, EffectContextFlagPendingNextFunctionIsEffectThunk|EffectContextFlagPendingNextObjectTryPropertyIsEffectThunk)
				}
			}
		}

		// Function-like nodes create a new scope, so they should not directly inherit
		// yieldability from an outer Effect scope. Matching Effect helpers re-enable the
		// flag on the specific body node below.
		if ast.IsFunctionLike(node) {
			node.ForEachChild(resetChildFunctionScopeFlags)
		}

		// reset stores correlated to a flag set here.
		if *links.EffectContextFlags.Get(node)&EffectContextFlagCanYieldEffect == 0 {
			*links.EffectYieldGeneratorFunction.Get(node) = nil
		}

		node.ForEachChild(walk)
		return false
	}
	walk(sf.AsNode())
}
