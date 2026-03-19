package typeparser

import (
	"sync"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
)

type EffectLinksRoot struct {
	TypeAtLocation core.LinkStore[*ast.Node, *checker.Type]

	mu    sync.Mutex
	extra map[string]any
}

func getEffectLinksRoot(c *checker.Checker) *EffectLinksRoot {
	if c.EffectLinks == nil {
		c.EffectLinks = &EffectLinksRoot{}
	}
	return c.EffectLinks.(*EffectLinksRoot)
}

func getOrCreateExtra[T any](c *checker.Checker, key string, init func() *T) *T {
	root := getEffectLinksRoot(c)
	root.mu.Lock()
	defer root.mu.Unlock()

	if root.extra == nil {
		root.extra = make(map[string]any)
	}
	if existing, ok := root.extra[key]; ok {
		return existing.(*T)
	}

	value := init()
	root.extra[key] = value
	return value
}
