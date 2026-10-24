package ast

import (
	"github.com/microsoft/typescript-go/internal/ast"
	"github.com/microsoft/typescript-go/internal/tspath"
)

func SourceFilePath(sf *ast.SourceFile) tspath.Path { return sf.Path() }
