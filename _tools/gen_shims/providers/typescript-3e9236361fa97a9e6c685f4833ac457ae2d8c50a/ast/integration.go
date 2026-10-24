package ast

import (
	"github.com/microsoft/typescript-go/internal/ast"
	"github.com/microsoft/typescript-go/internal/tspath"
)

func SourceFilePath(sf *ast.SourceFile) tspath.PathKey { return sf.PathKey() }

func NewSourceFileParseOptions(fileName tspath.RootedFilePath, path tspath.PathKey) ast.SourceFileParseOptions {
	return ast.SourceFileParseOptions{FileName: fileName, PathKey: path}
}
