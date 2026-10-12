package customlint

import (
	"go/ast"
	"go/token"
	"go/types"
	"strings"

	"golang.org/x/tools/go/analysis"
)

const typeParserPackage = "github.com/effect-ts/tsgo/internal/typeparser"

var checkerLocationMethods = map[string]string{
	"GetSymbolAtLocation": "GetSymbolAtLocation",
	"GetTypeAtLocation":   "getTypeAtLocationUncached",
}

var checkerLocationAnalyzer = &analysis.Analyzer{
	Name: "checkerlocation",
	Doc:  "require TypeParser wrappers for compiler symbol and type lookup in internal packages",
	Run:  runCheckerLocation,
}

func runCheckerLocation(pass *analysis.Pass) (any, error) {
	if !strings.HasPrefix(pass.Pkg.Path(), "github.com/effect-ts/tsgo/internal/") {
		return nil, nil
	}
	allowed := make(map[*ast.SelectorExpr]bool)
	for methodName, forwardingMethod := range checkerLocationMethods {
		if selector := forwardingSelector(pass, methodName, forwardingMethod); selector != nil {
			allowed[selector] = true
		}
	}
	for _, file := range pass.Files {
		ast.Inspect(file, func(node ast.Node) bool {
			selector, ok := node.(*ast.SelectorExpr)
			if ok && !allowed[selector] {
				if selection := pass.TypesInfo.Selections[selector]; selection != nil && isCompilerLocationMethod(selection.Obj()) {
					name := selection.Obj().Name()
					pass.Reportf(selector.Pos(), "use TypeParser.%s instead of compiler Checker.%s", name, name)
				}
			}
			return true
		})
	}
	return nil, nil
}

func receiverType(t types.Type) *types.Named {
	t = types.Unalias(t)
	if pointer, ok := t.(*types.Pointer); ok {
		t = types.Unalias(pointer.Elem())
	}
	named, _ := t.(*types.Named)
	return named
}

func isCompilerLocationMethod(object types.Object) bool {
	method, ok := object.(*types.Func)
	if !ok || checkerLocationMethods[method.Name()] == "" {
		return false
	}
	signature := method.Type().(*types.Signature)
	if signature.Recv() == nil {
		return false
	}
	receiver := receiverType(signature.Recv().Type())
	if receiver == nil || receiver.Obj().Name() != "Checker" || receiver.Obj().Pkg() != method.Pkg() || method.Pkg() == nil {
		return false
	}
	switch method.Pkg().Path() {
	case "github.com/microsoft/TypeScript/tsc/internal/checker", "github.com/microsoft/typescript-go/internal/checker":
		return true
	default:
		return false
	}
}

func forwardingSelector(pass *analysis.Pass, methodName, forwardingMethod string) *ast.SelectorExpr {
	if pass.Pkg.Path() != typeParserPackage {
		return nil
	}
	for _, file := range pass.Files {
		for _, declaration := range file.Decls {
			method, ok := declaration.(*ast.FuncDecl)
			if !ok || method.Recv == nil || method.Name.Name != forwardingMethod || method.Body == nil || len(method.Body.List) == 0 {
				continue
			}
			object, ok := pass.TypesInfo.Defs[method.Name].(*types.Func)
			if !ok {
				continue
			}
			signature := object.Type().(*types.Signature)
			receiver := receiverType(signature.Recv().Type())
			if receiver == nil || receiver.Obj() != pass.Pkg.Scope().Lookup("TypeParser") || signature.Params().Len() != 1 || signature.Results().Len() != 1 || signature.Variadic() {
				continue
			}
			result, ok := method.Body.List[len(method.Body.List)-1].(*ast.ReturnStmt)
			if !ok || len(result.Results) != 1 {
				continue
			}
			call, ok := result.Results[0].(*ast.CallExpr)
			if !ok || len(call.Args) != 1 || call.Ellipsis != token.NoPos {
				continue
			}
			selector, ok := call.Fun.(*ast.SelectorExpr)
			if !ok {
				continue
			}
			selection := pass.TypesInfo.Selections[selector]
			if selection == nil || selection.Obj().Name() != methodName || !isCompilerLocationMethod(selection.Obj()) {
				continue
			}
			field, ok := selector.X.(*ast.SelectorExpr)
			if !ok {
				continue
			}
			fieldSelection := pass.TypesInfo.Selections[field]
			structure, ok := receiver.Underlying().(*types.Struct)
			if !ok || fieldSelection == nil || len(fieldSelection.Index()) != 1 || fieldSelection.Obj().Name() != "checker" || structure.Field(fieldSelection.Index()[0]) != fieldSelection.Obj() {
				continue
			}
			receiverID, receiverOK := field.X.(*ast.Ident)
			argumentID, argumentOK := call.Args[0].(*ast.Ident)
			if receiverOK && argumentOK && pass.TypesInfo.ObjectOf(receiverID) == signature.Recv() && pass.TypesInfo.ObjectOf(argumentID) == signature.Params().At(0) {
				return selector
			}
		}
	}
	return nil
}
