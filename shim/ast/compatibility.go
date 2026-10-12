package ast

import "github.com/microsoft/TypeScript/tsc/internal/ast"

func TSGetSymbolName(symbol *ast.Symbol) string { return symbol.Name() }

func TSGetSymbolDeclarations(symbol *ast.Symbol) []*ast.Node { return symbol.Declarations() }

func TSGetSymbolFlags(symbol *ast.Symbol) ast.SymbolFlags { return symbol.Flags() }

func TSGetSymbolCheckFlags(symbol *ast.Symbol) ast.CheckFlags { return symbol.CheckFlags() }

func TSGetSymbolValueDeclaration(symbol *ast.Symbol) *ast.Node { return symbol.ValueDeclaration() }

func TSGetSymbolMembers(symbol *ast.Symbol) ast.SymbolTable { return symbol.Members() }

func TSGetSymbolExports(symbol *ast.Symbol) ast.SymbolTable { return symbol.Exports() }

func TSGetSymbolParent(symbol *ast.Symbol) *ast.Symbol { return symbol.Parent() }

func TSGetSymbolExportSymbol(symbol *ast.Symbol) *ast.Symbol { return symbol.ExportSymbol() }
