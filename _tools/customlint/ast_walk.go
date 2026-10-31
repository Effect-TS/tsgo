package customlint

import (
	"go/ast"
	"go/token"
	"go/types"
	"strings"

	"golang.org/x/tools/go/analysis"
)

// astWalkPackages are the packages whose detection should start from piping
// flows (docs/agents/piping-flows.md) rather than hand-written AST walks.
var astWalkPackages = map[string]bool{
	"github.com/effect-ts/tsgo/internal/rules":    true,
	"github.com/effect-ts/tsgo/internal/fixables": true,
}

const astWalkDirective = "//ets:ast-walk"

var astWalkAnalyzer = &analysis.Analyzer{
	Name: "astwalk",
	Doc:  "require an //ets:ast-walk <reason> comment on AST walks in rules and fixables, which should iterate piping flows where they can",
	Run:  runASTWalk,
}

// astWalkComment is one //ets:ast-walk directive and whether it covers a walk.
type astWalkComment struct {
	comment *ast.Comment
	used    bool
}

func runASTWalk(pass *analysis.Pass) (any, error) {
	if !astWalkPackages[pass.Pkg.Path()] {
		return nil, nil
	}
	for _, file := range pass.Files {
		if strings.HasSuffix(pass.Fset.File(file.Pos()).Name(), "_test.go") {
			continue
		}
		// Directives by the line they are on, and by the last line of their
		// comment group, which is the line above the code they annotate.
		onLine := make(map[int]*astWalkComment)
		aboveLine := make(map[int]*astWalkComment)
		var directives []*astWalkComment
		for _, group := range file.Comments {
			for _, comment := range group.List {
				reason, ok := strings.CutPrefix(comment.Text, astWalkDirective)
				if !ok || reason != "" && reason[0] != ' ' {
					continue
				}
				if strings.TrimSpace(reason) == "" {
					pass.Reportf(comment.Pos(), "%s needs a reason: what does the walk find that piping flows do not produce?", astWalkDirective)
				}
				directive := &astWalkComment{comment: comment}
				directives = append(directives, directive)
				onLine[line(pass.Fset, comment.Pos())] = directive
				aboveLine[line(pass.Fset, group.End())+1] = directive
			}
		}
		var stack []ast.Node
		ast.Inspect(file, func(node ast.Node) bool {
			if node == nil {
				stack = stack[:len(stack)-1]
				return true
			}
			stack = append(stack, node)
			call, ok := node.(*ast.CallExpr)
			if !ok {
				return true
			}
			name := astWalkCall(pass, call)
			if name == "" {
				return true
			}
			if directive := onLine[line(pass.Fset, call.Pos())]; directive != nil {
				directive.used = true
				return true
			}
			for i := len(stack) - 1; i >= 0; i-- {
				if directive := aboveLine[line(pass.Fset, stack[i].Pos())]; directive != nil {
					directive.used = true
					return true
				}
			}
			pass.Reportf(call.Pos(), "%s walks the AST: match piping flows instead (docs/agents/piping-flows.md), or add %s <reason> when the walk finds something flows do not produce", name, astWalkDirective)
			return true
		})
		for _, directive := range directives {
			if !directive.used {
				pass.Reportf(directive.comment.Pos(), "%s annotates no AST walk: remove it", astWalkDirective)
			}
		}
	}
	return nil, nil
}

func line(fset *token.FileSet, pos token.Pos) int {
	return fset.Position(pos).Line
}

// astWalkCall names the compiler AST walk that call starts, or returns "".
func astWalkCall(pass *analysis.Pass, call *ast.CallExpr) string {
	var identifier *ast.Ident
	switch callee := ast.Unparen(call.Fun).(type) {
	case *ast.SelectorExpr:
		identifier = callee.Sel
	case *ast.Ident:
		identifier = callee
	default:
		return ""
	}
	function, ok := pass.TypesInfo.Uses[identifier].(*types.Func)
	if !ok || function.Pkg() == nil || !isCompilerASTPackage(function.Pkg().Path()) {
		return ""
	}
	signature := function.Type().(*types.Signature)
	switch {
	case function.Name() == "ForEachChild" && signature.Recv() != nil:
		if receiver := receiverType(signature.Recv().Type()); receiver != nil && receiver.Obj().Name() == "Node" {
			return "Node.ForEachChild"
		}
	case function.Name() == "NewNodeVisitor" && signature.Recv() == nil:
		return "ast.NewNodeVisitor"
	}
	return ""
}

func isCompilerASTPackage(path string) bool {
	switch path {
	case "github.com/microsoft/TypeScript/tsc/internal/ast", "github.com/microsoft/TypeScript/tsc/shim/ast",
		"github.com/microsoft/typescript-go/internal/ast", "github.com/microsoft/typescript-go/shim/ast":
		return true
	default:
		return false
	}
}
