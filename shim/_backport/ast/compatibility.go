package ast

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/tspath"
)

func SourceFilePath(sf *ast.SourceFile) tspath.Path { return sf.Path() }

func NewSourceFileParseOptions(fileName string, path tspath.Path) ast.SourceFileParseOptions {
	return ast.SourceFileParseOptions{FileName: fileName, Path: path}
}
