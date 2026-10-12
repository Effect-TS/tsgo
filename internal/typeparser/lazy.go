package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

// lazy resolves a value on its first read and returns the stored value on
// later reads. Result structs hold it by pointer so their copies share one
// resolution. A checker runs on a single goroutine, so no lock is needed.
type lazy[T any] struct {
	resolve func() T
	value   T
}

func newLazy[T any](resolve func() T) *lazy[T] {
	return &lazy[T]{resolve: resolve}
}

func resolvedLazy[T any](value T) *lazy[T] {
	return &lazy[T]{value: value}
}

func (l *lazy[T]) get() T {
	if l == nil {
		var zero T
		return zero
	}
	if l.resolve != nil {
		l.value = l.resolve()
		l.resolve = nil
	}
	return l.value
}

func (tp *TypeParser) lazyTypeAtLocation(node *ast.Node) *lazy[*checker.Type] {
	return newLazy(func() *checker.Type { return tp.GetTypeAtLocation(node) })
}

func (tp *TypeParser) lazyCallReturnType(call *ast.Node) *lazy[*checker.Type] {
	return newLazy(func() *checker.Type {
		if signature := tp.checker.GetResolvedSignature(call); signature != nil {
			return tp.checker.GetReturnTypeOfSignature(signature)
		}
		return nil
	})
}
