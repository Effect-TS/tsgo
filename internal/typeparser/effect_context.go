package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
)

type EffectContextFlags uint8

const (
	EffectContextFlagNone           EffectContextFlags = 0
	EffectContextFlagCanYieldEffect EffectContextFlags = 1 << iota
)

func (tp *TypeParser) GetEffectContextFlags(node *ast.Node) EffectContextFlags {
	if tp == nil {
		return EffectContextFlagNone
	}
	links := ensureEffectContextAnalyzed(tp.checker, node)
	if links == nil {
		return EffectContextFlagNone
	}

	if closest, ok := getClosestNodeWithLinks(&links.EffectContextFlags, node); ok {
		return *links.EffectContextFlags.TryGet(closest)
	}
	return EffectContextFlagNone
}

func (tp *TypeParser) GetEffectYieldGeneratorFunction(node *ast.Node) *ast.FunctionExpression {
	if tp == nil {
		return nil
	}
	links := ensureEffectContextAnalyzed(tp.checker, node)
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

func ensureEffectContextAnalyzed(c *checker.Checker, node *ast.Node) *EffectLinks {
	if c == nil || node == nil {
		return nil
	}

	links := (&TypeParser{program: c.Program(), checker: c}).GetEffectLinks()

	if links.EffectContextFlags.Has(node) {
		return links
	}

	sf := ast.GetSourceFileOfNode(node)
	if sf == nil {
		return nil
	}

	Cached(&links.EffectContextAnalyzed, sf, func() bool {
		analyzeEffectContextForSourceFile(c, sf)
		return true
	})
	return links
}

func analyzeEffectContextForSourceFile(c *checker.Checker, sf *ast.SourceFile) {
	if c == nil || sf == nil {
		return
	}

	links := (&TypeParser{program: c.Program(), checker: c}).GetEffectLinks()

	var walk ast.Visitor
	var pendingEnableFlags core.LinkStore[*ast.Node, EffectContextFlags]
	var pendingDisableFlags core.LinkStore[*ast.Node, EffectContextFlags]

	resetChildCanYieldEffect := func(node *ast.Node) bool {
		*pendingDisableFlags.Get(node) |= EffectContextFlagCanYieldEffect
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

		// logic for this node
		tp := &TypeParser{program: c.Program(), checker: c}
		if effectGen := tp.EffectGenCall(node); effectGen != nil {
			bodyNode := effectGen.Body.AsNode()
			*pendingEnableFlags.Get(bodyNode) |= EffectContextFlagCanYieldEffect
			*links.EffectYieldGeneratorFunction.Get(bodyNode) = effectGen.GeneratorFunction
		} else if effectFn := tp.EffectFnGenCall(node); effectFn != nil {
			bodyNode := effectFn.Body.AsNode()
			*pendingEnableFlags.Get(bodyNode) |= EffectContextFlagCanYieldEffect
			*links.EffectYieldGeneratorFunction.Get(bodyNode) = effectFn.GeneratorFunction
		} else if effectFn := tp.EffectFnUntracedGenCall(node); effectFn != nil {
			bodyNode := effectFn.Body.AsNode()
			*pendingEnableFlags.Get(bodyNode) |= EffectContextFlagCanYieldEffect
			*links.EffectYieldGeneratorFunction.Get(bodyNode) = effectFn.GeneratorFunction
		} else if effectFn := tp.EffectFnUntracedEagerGenCall(node); effectFn != nil {
			bodyNode := effectFn.Body.AsNode()
			*pendingEnableFlags.Get(bodyNode) |= EffectContextFlagCanYieldEffect
			*links.EffectYieldGeneratorFunction.Get(bodyNode) = effectFn.GeneratorFunction
		}

		// Function-like nodes create a new scope, so they should not directly inherit
		// yieldability from an outer Effect scope. Matching Effect helpers re-enable the
		// flag on the specific body node below.
		if ast.IsFunctionLike(node) {
			node.ForEachChild(resetChildCanYieldEffect)
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
