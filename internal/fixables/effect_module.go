package fixables

import (
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
)

// effectModuleMethod builds <module>.method, where <module> is the reference
// to the Effect module from the file's imports.
func effectModuleMethod(tracker *rewriter.Tracker, sf *ast.SourceFile, method string) *ast.Node {
	return tracker.NewPropertyAccessPath(typeparser.FindEffectModuleReference(sf).Append(method)...)
}

// replaceEffectMethodCallee replaces the whole callee with <module>.method.
func replaceEffectMethodCallee(tracker *rewriter.Tracker, sf *ast.SourceFile, callee *ast.Node, method string) {
	tracker.ReplaceNode(sf, callee, effectModuleMethod(tracker, sf, method), nil)
}
