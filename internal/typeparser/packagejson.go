// Package typeparser provides Effect type detection and parsing utilities.
package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/packagejson"
	"github.com/microsoft/typescript-go/shim/tspath"
)

type packageJsonProgram interface {
	GetSourceFileMetaData(path tspath.Path) ast.SourceFileMetaData
	GetPackageJsonInfo(pkgJsonPath string) *packagejson.InfoCacheEntry
}

// PackageJsonForSourceFile returns the nearest package.json contents for a source file, or nil.
func PackageJsonForSourceFile(c *checker.Checker, sf *ast.SourceFile) *packagejson.PackageJson {
	if c == nil || sf == nil {
		return nil
	}

	prog, ok := c.Program().(packageJsonProgram)
	if !ok || prog == nil {
		return nil
	}

	meta := prog.GetSourceFileMetaData(sf.Path())
	if meta.PackageJsonDirectory == "" {
		return nil
	}

	packageJsonPath := tspath.CombinePaths(meta.PackageJsonDirectory, "package.json")
	info := prog.GetPackageJsonInfo(packageJsonPath)
	if info == nil {
		return nil
	}
	return info.GetContents()
}
