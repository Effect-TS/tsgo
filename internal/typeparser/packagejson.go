// Package typeparser provides Effect type detection and parsing utilities.
package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/packagejson"
	"github.com/microsoft/TypeScript/tsc/shim/tspath"
)

type packageJsonProgram interface {
	GetSourceFileMetaData(path tspath.PathKey) ast.SourceFileMetaData
	GetPackageJsonInfo(pkgJsonPath tspath.RootedFilePath) *packagejson.InfoCacheEntry
}

// PackageJsonForSourceFile returns the nearest package.json contents for a source file, or nil.
// Results are cached per source file on EffectLinks for the checker's lifetime.
func (tp *TypeParser) PackageJsonForSourceFile(sf *ast.SourceFile) *packagejson.PackageJson {
	if tp == nil || tp.checker == nil || sf == nil {
		return nil
	}

	return Cached(&tp.links.PackageJsonForSourceFile, sf, func() *packagejson.PackageJson {
		prog, ok := tp.program.(packageJsonProgram)
		if !ok || prog == nil {
			return nil
		}

		meta := prog.GetSourceFileMetaData(ast.SourceFilePath(sf))
		if meta.PackageJsonDirectory == "" {
			return nil
		}

		packageJsonPath := tspath.CombinePaths(string(meta.PackageJsonDirectory), "package.json")
		info := prog.GetPackageJsonInfo(tspath.RootedFilePath(packageJsonPath))
		if info == nil {
			return nil
		}
		return info.GetContents()
	})
}
