package customlint

import (
	"go/ast"
	"go/token"
	"go/types"
	"strings"

	"golang.org/x/tools/go/analysis"
)

// typeAtLocationParsers maps the TypeParser methods that parse an Effect
// library type to the AtLocation method that should replace them when their
// argument comes from TypeParser.GetTypeAtLocation.
var typeAtLocationParsers = map[string]string{
	"EffectType":          "EffectTypeAtLocation",
	"StrictEffectType":    "StrictEffectTypeAtLocation",
	"EffectSubtype":       "EffectSubtypeAtLocation",
	"FiberType":           "FiberTypeAtLocation",
	"HasEffectTypeId":     "HasEffectTypeIdAtLocation",
	"EffectYieldableType": "EffectYieldableTypeAtLocation",
	"StreamType":          "StreamTypeAtLocation",
	"LayerType":           "LayerTypeAtLocation",
	"ServiceType":         "ServiceTypeAtLocation",
	"ContextTag":          "ContextTagAtLocation",
	"EffectSchemaTypes":   "EffectSchemaTypesAtLocation",
}

var typeAtLocationAnalyzer = &analysis.Analyzer{
	Name: "typeatlocation",
	Doc:  "require TypeParser AtLocation parsers instead of parsing the result of TypeParser.GetTypeAtLocation",
	Run:  runTypeAtLocation,
}

func runTypeAtLocation(pass *analysis.Pass) (any, error) {
	if !strings.HasPrefix(pass.Pkg.Path(), "github.com/effect-ts/tsgo/") || pass.Pkg.Path() == typeParserPackage {
		return nil, nil
	}
	sources := typeAtLocationVariables(pass)
	for _, file := range pass.Files {
		ast.Inspect(file, func(node ast.Node) bool {
			call, ok := node.(*ast.CallExpr)
			if !ok || len(call.Args) != 1 {
				return true
			}
			name := typeParserMethod(pass, call)
			replacement := typeAtLocationParsers[name]
			if replacement == "" {
				return true
			}
			argument := ast.Unparen(call.Args[0])
			fromLocation := isTypeAtLocationCall(pass, argument)
			if identifier, ok := argument.(*ast.Ident); ok {
				variable, _ := pass.TypesInfo.ObjectOf(identifier).(*types.Var)
				fromLocation = sources[variable]
			}
			if fromLocation {
				pass.Reportf(call.Pos(), "use TypeParser.%s instead of TypeParser.%s on TypeParser.GetTypeAtLocation", replacement, name)
			}
			return true
		})
	}
	return nil, nil
}

// typeAtLocationVariables reports, for each variable assigned in the package,
// whether every assigned value is a TypeParser.GetTypeAtLocation call and
// every use could be replaced by an AtLocation parser: being the argument of
// a parser that has one, or being compared with nil.
func typeAtLocationVariables(pass *analysis.Pass) map[*types.Var]bool {
	sources := typeAtLocationAssignments(pass)
	for _, file := range pass.Files {
		var stack []ast.Node
		ast.Inspect(file, func(node ast.Node) bool {
			if node == nil {
				stack = stack[:len(stack)-1]
				return true
			}
			if identifier, ok := node.(*ast.Ident); ok {
				variable, _ := pass.TypesInfo.Uses[identifier].(*types.Var)
				if sources[variable] && !isReplaceableTypeUse(pass, identifier, stack) {
					sources[variable] = false
				}
			}
			stack = append(stack, node)
			return true
		})
	}
	return sources
}

// isReplaceableTypeUse reports whether identifier, whose ancestors are stack,
// is assigned to, compared with nil, or the argument of an AtLocation-able parser.
func isReplaceableTypeUse(pass *analysis.Pass, identifier *ast.Ident, stack []ast.Node) bool {
	var child ast.Node = identifier
	i := len(stack) - 1
	for i >= 0 {
		if _, ok := stack[i].(*ast.ParenExpr); !ok {
			break
		}
		child = stack[i]
		i--
	}
	if i < 0 {
		return false
	}
	switch parent := stack[i].(type) {
	case *ast.AssignStmt:
		for _, target := range parent.Lhs {
			if target == child {
				return true
			}
		}
	case *ast.BinaryExpr:
		other := parent.X
		if other == child {
			other = parent.Y
		}
		if nilIdentifier, ok := ast.Unparen(other).(*ast.Ident); ok && (parent.Op == token.EQL || parent.Op == token.NEQ) {
			_, isNil := pass.TypesInfo.Uses[nilIdentifier].(*types.Nil)
			return isNil
		}
	case *ast.CallExpr:
		return len(parent.Args) == 1 && parent.Args[0] == child && typeAtLocationParsers[typeParserMethod(pass, parent)] != ""
	}
	return false
}

// typeAtLocationAssignments reports, for each variable assigned in the
// package, whether every assigned value is a TypeParser.GetTypeAtLocation call.
func typeAtLocationAssignments(pass *analysis.Pass) map[*types.Var]bool {
	sources := make(map[*types.Var]bool)
	record := func(lhs []ast.Expr, rhs []ast.Expr) {
		for i, target := range lhs {
			identifier, ok := target.(*ast.Ident)
			if !ok {
				continue
			}
			variable, ok := pass.TypesInfo.ObjectOf(identifier).(*types.Var)
			if !ok {
				continue
			}
			fromLocation := len(lhs) == len(rhs) && isTypeAtLocationCall(pass, ast.Unparen(rhs[i]))
			previous, seen := sources[variable]
			sources[variable] = fromLocation && (!seen || previous)
		}
	}
	for _, file := range pass.Files {
		ast.Inspect(file, func(node ast.Node) bool {
			switch node := node.(type) {
			case *ast.AssignStmt:
				record(node.Lhs, node.Rhs)
			case *ast.ValueSpec:
				if len(node.Values) > 0 {
					names := make([]ast.Expr, len(node.Names))
					for i, name := range node.Names {
						names[i] = name
					}
					record(names, node.Values)
				}
			}
			return true
		})
	}
	return sources
}

func isTypeAtLocationCall(pass *analysis.Pass, expression ast.Expr) bool {
	call, ok := expression.(*ast.CallExpr)
	return ok && typeParserMethod(pass, call) == "GetTypeAtLocation"
}

// typeParserMethod returns the name of the typeparser.TypeParser method that
// call invokes, or "" when it invokes anything else.
func typeParserMethod(pass *analysis.Pass, call *ast.CallExpr) string {
	selector, ok := call.Fun.(*ast.SelectorExpr)
	if !ok {
		return ""
	}
	selection := pass.TypesInfo.Selections[selector]
	if selection == nil || selection.Kind() != types.MethodVal {
		return ""
	}
	receiver := receiverType(selection.Recv())
	if receiver == nil || receiver.Obj().Name() != "TypeParser" || receiver.Obj().Pkg() == nil || receiver.Obj().Pkg().Path() != typeParserPackage {
		return ""
	}
	return selection.Obj().Name()
}
