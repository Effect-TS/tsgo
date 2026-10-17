package customlint

import (
	"go/ast"
	"go/types"
	"strings"

	"golang.org/x/tools/go/analysis"
)

var rawSymbolMembers = map[string]bool{
	"Name": true, "Declarations": true, "Flags": true, "CheckFlags": true,
	"ValueDeclaration": true, "Members": true, "Exports": true, "Parent": true, "ExportSymbol": true,
}

var symbolAccessAnalyzer = &analysis.Analyzer{
	Name: "symbolaccess",
	Doc:  "require compatibility helpers for raw compiler Symbol access",
	Run:  runSymbolAccess,
}

func runSymbolAccess(pass *analysis.Pass) (any, error) {
	if !strings.HasPrefix(pass.Pkg.Path(), "github.com/effect-ts/tsgo/") {
		return nil, nil
	}
	for _, file := range pass.Files {
		ast.Inspect(file, func(node ast.Node) bool {
			selector, ok := node.(*ast.SelectorExpr)
			if !ok {
				return true
			}
			selection := pass.TypesInfo.Selections[selector]
			if selection == nil || !isRawSymbolMember(selection) {
				return true
			}
			name := selection.Obj().Name()
			pass.Reportf(selector.Pos(), "use ast.TSGetSymbol%s instead of raw compiler Symbol.%s access", name, name)
			return true
		})
	}
	return nil, nil
}

func isCompilerSymbol(t types.Type) bool {
	named := receiverType(t)
	if named == nil || named.Obj().Name() != "Symbol" || named.Obj().Pkg() == nil {
		return false
	}
	switch named.Obj().Pkg().Path() {
	case "github.com/microsoft/TypeScript/tsc/internal/ast", "github.com/microsoft/typescript-go/internal/ast":
		return true
	default:
		return false
	}
}

func isRawSymbolMember(selection *types.Selection) bool {
	if !rawSymbolMembers[selection.Obj().Name()] {
		return false
	}
	switch object := selection.Obj().(type) {
	case *types.Func:
		signature := object.Type().(*types.Signature)
		return signature.Recv() != nil && isCompilerSymbol(signature.Recv().Type())
	case *types.Var:
		owner := selection.Recv()
		for _, index := range selection.Index()[:len(selection.Index())-1] {
			owner = types.Unalias(owner)
			if pointer, ok := owner.(*types.Pointer); ok {
				owner = types.Unalias(pointer.Elem())
			}
			structure, ok := owner.Underlying().(*types.Struct)
			if !ok {
				return false
			}
			owner = structure.Field(index).Type()
		}
		return object.IsField() && isCompilerSymbol(owner)
	default:
		return false
	}
}
