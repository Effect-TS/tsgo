package tsoptions

import (
	"github.com/microsoft/TypeScript/tsc/providershim/ast"
	"github.com/microsoft/TypeScript/tsc/providershim/collections"
	"github.com/microsoft/TypeScript/tsc/providershim/core"
	"github.com/microsoft/TypeScript/tsc/providershim/tsoptions"
	"github.com/microsoft/TypeScript/tsc/providershim/tspath"
	"github.com/microsoft/TypeScript/tsc/providershim/vfs"
)

type ParseConfigHost interface {
	FS() vfs.FS
	IsContentMapperPackageAllowed(packageDirectory tspath.RootedDirectoryPath) bool
}

type defaultParseConfigHost struct {
	fs vfs.FS
}

func NewDefaultParseConfigHost(fs vfs.FS) ParseConfigHost { return &defaultParseConfigHost{fs} }

func (h *defaultParseConfigHost) FS() vfs.FS { return h.fs }

func (h *defaultParseConfigHost) IsContentMapperPackageAllowed(tspath.RootedDirectoryPath) bool {
	return true
}

func ParseJsonConfigFileContent(json any, host ParseConfigHost, basePath tspath.RootedDirectoryPath, existingOptions *core.CompilerOptions, configFileName tspath.RootedFilePath, resolutionStack []tspath.PathKey, extendedConfigCache tsoptions.ExtendedConfigCache) *tsoptions.ParsedCommandLine {
	return tsoptions.ParseJsonConfigFileContent(json, host.FS(), basePath, existingOptions, configFileName, resolutionStack, extendedConfigCache)
}

func ParseJsonSourceFileConfigFileContent(sourceFile *tsoptions.TsConfigSourceFile, host ParseConfigHost, basePath tspath.RootedDirectoryPath, existingOptions *core.CompilerOptions, existingOptionsRaw *collections.OrderedMap[string, any], resolutionStack []tspath.PathKey, extendedConfigCache tsoptions.ExtendedConfigCache) *tsoptions.ParsedCommandLine {
	return tsoptions.ParseJsonSourceFileConfigFileContent(sourceFile, host.FS(), basePath, existingOptions, existingOptionsRaw, resolutionStack, extendedConfigCache)
}

func GetParsedCommandLineOfConfigFile(configFileName tspath.RootedFilePath, options *core.CompilerOptions, optionsRaw *collections.OrderedMap[string, any], host ParseConfigHost, extendedConfigCache tsoptions.ExtendedConfigCache) (*tsoptions.ParsedCommandLine, []*ast.Diagnostic) {
	return tsoptions.GetParsedCommandLineOfConfigFile(configFileName, options, optionsRaw, host.FS(), extendedConfigCache)
}

func GetParsedCommandLineOfConfigFilePath(configFileName tspath.RootedFilePath, path tspath.PathKey, options *core.CompilerOptions, optionsRaw *collections.OrderedMap[string, any], host ParseConfigHost, extendedConfigCache tsoptions.ExtendedConfigCache) (*tsoptions.ParsedCommandLine, []*ast.Diagnostic) {
	return tsoptions.GetParsedCommandLineOfConfigFilePath(configFileName, path, options, optionsRaw, host.FS(), extendedConfigCache)
}
