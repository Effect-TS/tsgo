// Package typeparser provides Effect type detection and parsing utilities.
package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// ScopeKind represents the kind of innermost scope found by FindEnclosingScopes.
type ScopeKind int

const (
	ScopeKindSourceFile ScopeKind = iota
	ScopeKindFunction
	ScopeKindEffectGen
	ScopeKindEffectFn
)

// EnclosingScopes represents the nearest function scope and Effect scope in the parent chain.
type EnclosingScopes struct {
	ScopeNode   *ast.Node
	EffectGen   *EffectGenCallResult
	EffectFnGen *EffectGenCallResult
	ScopeKind   ScopeKind
}

// EffectGeneratorFunction returns the generator function from whichever Effect scope is set.
func (s *EnclosingScopes) EffectGeneratorFunction() *ast.FunctionExpression {
	if s.EffectGen != nil {
		return s.EffectGen.GeneratorFunction
	}
	if s.EffectFnGen != nil {
		return s.EffectFnGen.GeneratorFunction
	}
	return nil
}

// FindEnclosingScopes walks parents of startNode to find the nearest function scope and Effect scope.
func FindEnclosingScopes(c *checker.Checker, startNode *ast.Node) EnclosingScopes {
	if c == nil || startNode == nil {
		var result EnclosingScopes
		if startNode != nil {
			result.ScopeNode = ast.GetContainingFunction(startNode)
			if result.ScopeNode != nil {
				result.ScopeKind = ScopeKindFunction
			} else {
				result.ScopeKind = ScopeKindSourceFile
			}
		}
		return result
	}
	links := GetEffectLinks(c)
	return Cached(&links.FindEnclosingScopes, startNode, func() EnclosingScopes {
		var result EnclosingScopes
		result.ScopeNode = ast.GetContainingFunction(startNode)

		for current := startNode.Parent; current != nil; current = current.Parent {
			if effectGen := EffectGenCall(c, current); effectGen != nil {
				result.EffectGen = effectGen
				break
			} else if effectFnGen := EffectFnGenCall(c, current); effectFnGen != nil {
				result.EffectFnGen = effectFnGen
				break
			} else if effectFnGen := EffectFnUntracedGenCall(c, current); effectFnGen != nil {
				result.EffectFnGen = effectFnGen
				break
			} else if effectFnGen := EffectFnUntracedEagerGenCall(c, current); effectFnGen != nil {
				result.EffectFnGen = effectFnGen
				break
			}
		}

		// Derive ScopeKind
		if result.EffectGen != nil {
			result.ScopeKind = ScopeKindEffectGen
		} else if result.EffectFnGen != nil {
			result.ScopeKind = ScopeKindEffectFn
		} else if result.ScopeNode != nil {
			result.ScopeKind = ScopeKindFunction
		} else {
			result.ScopeKind = ScopeKindSourceFile
		}

		return result
	})
}
