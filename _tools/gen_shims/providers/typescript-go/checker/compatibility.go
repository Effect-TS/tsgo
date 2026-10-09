package checker

import "github.com/microsoft/typescript-go/internal/checker"

// GetNonDistributedTypeParameter is an identity operation for compiler
// versions that do not model distributed type parameters separately.
func GetNonDistributedTypeParameter(_ *checker.Checker, t *checker.Type) *checker.Type {
	return t
}
