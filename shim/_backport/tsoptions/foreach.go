package tsoptions

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/tsoptions"
)

// ForEachTsConfigPropArray forwards through a regular call because go:linkname
// does not support generic functions.
func ForEachTsConfigPropArray[T any](sourceFile *ast.SourceFile, propKey string, callback func(*ast.PropertyAssignment) *T) *T {
	return tsoptions.ForEachTsConfigPropArray(sourceFile, propKey, callback)
}
