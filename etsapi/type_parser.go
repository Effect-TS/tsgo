// Package etsapi exposes narrow public entry points for integrations that need
// stable access to tsgo functionality without importing internal packages.
package etsapi

import (
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// TypeParser wraps the internal type parser with a narrow public API.
type TypeParser struct {
	inner *typeparser.TypeParser
}

// Effect represents parsed Effect<A, E, R> type parameters.
type Effect struct {
	A *checker.Type
	E *checker.Type
	R *checker.Type
}

// Layer represents parsed Layer<ROut, E, RIn> type parameters.
type Layer struct {
	ROut *checker.Type
	E    *checker.Type
	RIn  *checker.Type
}

// NewTypeParser builds a checker-backed TypeParser.
func NewTypeParser(program checker.Program, checker *checker.Checker) *TypeParser {
	return &TypeParser{inner: typeparser.NewTypeParser(program, checker)}
}

// EffectType parses an Effect type and extracts A, E, R parameters.
func (tp *TypeParser) EffectType(t *checker.Type, atLocation *ast.Node) *Effect {
	if tp == nil || tp.inner == nil {
		return nil
	}
	return effectFromInternal(tp.inner.EffectType(t, atLocation))
}

// LayerType parses a Layer type and extracts ROut, E, RIn parameters.
func (tp *TypeParser) LayerType(t *checker.Type, atLocation *ast.Node) *Layer {
	if tp == nil || tp.inner == nil {
		return nil
	}
	return layerFromInternal(tp.inner.LayerType(t, atLocation))
}

// StreamType parses a Stream type and extracts A, E, R parameters.
func (tp *TypeParser) StreamType(t *checker.Type, atLocation *ast.Node) *Effect {
	if tp == nil || tp.inner == nil {
		return nil
	}
	return effectFromInternal(tp.inner.StreamType(t, atLocation))
}

// UnrollUnionMembers returns the constituent types of a union type,
// or a single-element slice containing the type itself if it's not a union.
func (tp *TypeParser) UnrollUnionMembers(t *checker.Type) []*checker.Type {
	if tp == nil || tp.inner == nil {
		return nil
	}
	return tp.inner.UnrollUnionMembers(t)
}

// IsYieldableErrorType reports whether the given type is assignable to Cause.YieldableError.
func (tp *TypeParser) IsYieldableErrorType(t *checker.Type) bool {
	if tp == nil || tp.inner == nil {
		return false
	}
	return tp.inner.IsYieldableErrorType(t)
}

func effectFromInternal(effect *typeparser.Effect) *Effect {
	if effect == nil {
		return nil
	}
	return &Effect{A: effect.A, E: effect.E, R: effect.R}
}

func layerFromInternal(layer *typeparser.Layer) *Layer {
	if layer == nil {
		return nil
	}
	return &Layer{ROut: layer.ROut, E: layer.E, RIn: layer.RIn}
}
