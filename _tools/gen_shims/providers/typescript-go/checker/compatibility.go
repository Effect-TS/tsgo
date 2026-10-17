package checker

import (
	"github.com/microsoft/typescript-go/internal/ast"
	"github.com/microsoft/typescript-go/internal/checker"
	"github.com/microsoft/typescript-go/internal/core"
)

// GetNonDistributedTypeParameter is an identity operation for compiler
// versions that do not model distributed type parameters separately.
func GetNonDistributedTypeParameter(_ *checker.Checker, t *checker.Type) *checker.Type {
	return t
}

// NodeLinkStore keeps node links in a map on compiler versions without
// block-allocated node ids. Their ids come from one global counter that
// concurrent checkers interleave, so a paged array would be mostly empty.
type NodeLinkStore[V any] struct {
	links core.LinkStore[*ast.Node, V]
}

func (s *NodeLinkStore[V]) Get(node *ast.Node) *V { return s.links.Get(node) }

func (s *NodeLinkStore[V]) Has(node *ast.Node) bool { return s.links.Has(node) }

func (s *NodeLinkStore[V]) TryGet(node *ast.Node) *V { return s.links.TryGet(node) }

// SymbolLinkStore keeps symbol links in a map, for the reason NodeLinkStore
// keeps node links in one.
type SymbolLinkStore[V any] struct {
	links core.LinkStore[*ast.Symbol, V]
}

func (s *SymbolLinkStore[V]) Get(symbol *ast.Symbol) *V { return s.links.Get(symbol) }

func (s *SymbolLinkStore[V]) Has(symbol *ast.Symbol) bool { return s.links.Has(symbol) }

func (s *SymbolLinkStore[V]) TryGet(symbol *ast.Symbol) *V { return s.links.TryGet(symbol) }
