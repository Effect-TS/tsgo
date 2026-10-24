package ast

import (
	"github.com/microsoft/typescript-go/internal/ast"
	"github.com/microsoft/typescript-go/internal/tspath"
)

func SourceFilePath(sf *ast.SourceFile) tspath.Path { return sf.Path() }

func NewSourceFileParseOptions(fileName string, path tspath.Path) ast.SourceFileParseOptions {
	return ast.SourceFileParseOptions{FileName: fileName, Path: path}
}

func TSGetSymbolName(symbol *ast.Symbol) string { return symbol.Name }

func TSGetSymbolDeclarations(symbol *ast.Symbol) []*ast.Node { return symbol.Declarations }

func TSGetSymbolFlags(symbol *ast.Symbol) ast.SymbolFlags { return symbol.Flags }

func TSGetSymbolCheckFlags(symbol *ast.Symbol) ast.CheckFlags { return symbol.CheckFlags }

func TSGetSymbolValueDeclaration(symbol *ast.Symbol) *ast.Node { return symbol.ValueDeclaration }

func TSGetSymbolMembers(symbol *ast.Symbol) ast.SymbolTable { return symbol.Members }

func TSGetSymbolExports(symbol *ast.Symbol) ast.SymbolTable { return symbol.Exports }

func TSGetSymbolParent(symbol *ast.Symbol) *ast.Symbol { return symbol.Parent }

func TSGetSymbolExportSymbol(symbol *ast.Symbol) *ast.Symbol { return symbol.ExportSymbol }
