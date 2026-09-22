package checker

import (
	"github.com/microsoft/TypeScript/tsc/internal/checker"
	_ "unsafe"
)

//go:linkname getNonDistributedTypeParameter github.com/microsoft/TypeScript/tsc/internal/checker.getNonDistributedTypeParameter
func getNonDistributedTypeParameter(t *checker.Type) *checker.Type

// GetNonDistributedTypeParameter returns the existing declaration binder for a
// distributed type parameter. It only reads compiler-owned types and never
// instantiates or allocates a replacement type.
func GetNonDistributedTypeParameter(_ *checker.Checker, t *checker.Type) *checker.Type {
	if t == nil {
		return nil
	}
	return getNonDistributedTypeParameter(t)
}
