package fixables

import (
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
)

func effectModuleMethod(tracker *rewriter.Tracker, sf *ast.SourceFile, receiver *ast.Node, method string) *ast.Node {
	if receiver == nil {
		receiver = tracker.NewIdentifier(typeparser.FindEffectModuleIdentifier(sf))
	} else {
		receiver = tracker.DeepCloneNode(receiver)
	}
	return tracker.NewPropertyAccessExpression(receiver, nil, tracker.NewIdentifier(method), ast.NodeFlagsNone)
}

func replaceEffectMethodCallee(tracker *rewriter.Tracker, sf *ast.SourceFile, callee *ast.Node, name *ast.Node, method string) {
	if name != nil {
		tracker.ReplaceNode(sf, name, tracker.NewIdentifier(method), nil)
		return
	}
	tracker.ReplaceNode(sf, callee, effectModuleMethod(tracker, sf, nil, method), nil)
}
