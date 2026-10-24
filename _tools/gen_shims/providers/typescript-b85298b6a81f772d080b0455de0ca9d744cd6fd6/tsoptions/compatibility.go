package tsoptions

import (
	shimpath "github.com/microsoft/TypeScript/tsc/shim/tspath"
	"github.com/microsoft/typescript-go/internal/collections"
	"github.com/microsoft/typescript-go/internal/core"
	"github.com/microsoft/typescript-go/internal/tsoptions"
	"github.com/microsoft/typescript-go/internal/tspath"
	"github.com/microsoft/typescript-go/internal/vfs"
)

type parseConfigHost struct {
	fs        vfs.FS
	directory string
}

func (h *parseConfigHost) FS() vfs.FS                  { return h.fs }
func (h *parseConfigHost) GetCurrentDirectory() string { return h.directory }

type ParseConfigHost interface {
	FS() vfs.FS
	IsContentMapperPackageAllowed(packageDirectory string) bool
}

type defaultParseConfigHost struct {
	fs vfs.FS
}

func NewDefaultParseConfigHost(fs vfs.FS) ParseConfigHost { return &defaultParseConfigHost{fs} }

func (h *defaultParseConfigHost) FS() vfs.FS                                { return h.fs }
func (h *defaultParseConfigHost) IsContentMapperPackageAllowed(string) bool { return true }

func ParseJsonConfigFileContent(json any, host ParseConfigHost, basePath string, existingOptions *core.CompilerOptions, configFileName string, resolutionStack []tspath.Path, cache tsoptions.ExtendedConfigCache) *tsoptions.ParsedCommandLine {
	return tsoptions.ParseJsonConfigFileContent(json, &parseConfigHost{host.FS(), basePath}, basePath, existingOptions, configFileName, resolutionStack, cache)
}

func ParseJsonSourceFileConfigFileContent(source *tsoptions.TsConfigSourceFile, host ParseConfigHost, basePath string, existingOptions *core.CompilerOptions, raw *collections.OrderedMap[string, any], resolutionStack []tspath.Path, cache tsoptions.ExtendedConfigCache) *tsoptions.ParsedCommandLine {
	return tsoptions.ParseJsonSourceFileConfigFileContent(source, &parseConfigHost{host.FS(), basePath}, basePath, existingOptions, raw, source.SourceFile.FileName(), resolutionStack, cache)
}

// NewParsedCommandLine adapts the explicit base directory and case sensitivity API.
func NewParsedCommandLine(options *core.CompilerOptions, files []string, references []*core.ProjectReference, directory string, sensitivity shimpath.CaseSensitivity) *tsoptions.ParsedCommandLine {
	return tsoptions.NewParsedCommandLine(options, files, references, tspath.ComparePathsOptions{
		CurrentDirectory:          directory,
		UseCaseSensitiveFileNames: sensitivity == shimpath.CaseSensitive,
	})
}
