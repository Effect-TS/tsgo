package customlint

import (
	"testing"

	"golang.org/x/tools/go/analysis/analysistest"
)

func TestASTWalk(t *testing.T) {
	dir, cleanup, err := analysistest.WriteFiles(map[string]string{
		"github.com/microsoft/TypeScript/tsc/internal/ast/ast.go": `package ast
type Node struct{}
type Visitor func(*Node) bool
func (n *Node) ForEachChild(v Visitor) bool { return false }
type NodeVisitor struct{}
func NewNodeVisitor(visit func(*Node) *Node) *NodeVisitor { return nil }`,
		"github.com/microsoft/TypeScript/tsc/shim/ast/shim.go": `package ast
import "github.com/microsoft/TypeScript/tsc/internal/ast"
type Node = ast.Node
func NewNodeVisitor(visit func(*ast.Node) *ast.Node) *ast.NodeVisitor { return ast.NewNodeVisitor(visit) }`,
		"github.com/effect-ts/tsgo/internal/rules/rule.go": `package rules
import "github.com/microsoft/TypeScript/tsc/shim/ast"
type Other struct{}
func (Other) ForEachChild(v func(*ast.Node) bool) bool { return false }
func unannotated(root *ast.Node) {
	var walk func(*ast.Node) bool
	walk = func(n *ast.Node) bool { return n.ForEachChild(walk) } // want "Node.ForEachChild walks the AST: match piping flows instead"
	root.ForEachChild(walk) // want "Node.ForEachChild walks the AST"
	_ = ast.NewNodeVisitor(nil) // want "ast.NewNodeVisitor walks the AST"
	Other{}.ForEachChild(nil)
}
func sameLine(root *ast.Node) {
	root.ForEachChild(nil) //ets:ast-walk finds yield expressions
}
func lineAbove(root *ast.Node) {
	// The class members are not flow steps.
	//ets:ast-walk finds class declarations
	root.ForEachChild(nil)
}
//ets:ast-walk scans the body for references to Scope
func wholeFunction(root *ast.Node) {
	var walk func(*ast.Node) bool
	walk = func(n *ast.Node) bool { return n.ForEachChild(walk) }
	root.ForEachChild(walk)
}
var Rule = struct{ Run func(*ast.Node) }{
	//ets:ast-walk finds new expressions
	Run: func(root *ast.Node) {
		if root != nil {
			root.ForEachChild(nil)
		}
	},
}
func missingReason(root *ast.Node) {
	/* want "needs a reason" */ //ets:ast-walk
	root.ForEachChild(nil)
}
func stale(root *ast.Node) {
	/* want "annotates no AST walk" */ //ets:ast-walk the walk was removed
	_ = root
}
func notADirective(root *ast.Node) {
	//ets:ast-walker is another word
	root.ForEachChild(nil) // want "Node.ForEachChild walks the AST"
}`,
		"github.com/effect-ts/tsgo/internal/rules/rule_test.go": `package rules
import "github.com/microsoft/TypeScript/tsc/shim/ast"
func inTest(root *ast.Node) { root.ForEachChild(nil) }`,
		"github.com/effect-ts/tsgo/internal/typeparser/parser.go": `package typeparser
import "github.com/microsoft/TypeScript/tsc/shim/ast"
func walks(root *ast.Node) { root.ForEachChild(nil) }`,
	})
	if err != nil {
		t.Fatal(err)
	}
	defer cleanup()
	analysistest.Run(t, dir, astWalkAnalyzer, "github.com/effect-ts/tsgo/internal/...")
}
