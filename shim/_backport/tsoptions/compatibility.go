package tsoptions

import (
	shimpath "github.com/microsoft/TypeScript/tsc/shim/tspath"
	"github.com/microsoft/typescript-go/shim/collections"
	"github.com/microsoft/typescript-go/shim/core"
	"github.com/microsoft/typescript-go/shim/tsoptions"
	"github.com/microsoft/typescript-go/shim/tspath"
	"github.com/microsoft/typescript-go/shim/vfs"
)

type ParsedOptions = core.ParsedOptions

type parseConfigHost struct {
	fs        vfs.FS
	directory string
}

func (h *parseConfigHost) FS() vfs.FS                  { return h.fs }
func (h *parseConfigHost) GetCurrentDirectory() string { return h.directory }

func ParseJsonConfigFileContent(json any, fs vfs.FS, basePath string, existingOptions *core.CompilerOptions, configFileName string, resolutionStack []tspath.Path, cache tsoptions.ExtendedConfigCache) *tsoptions.ParsedCommandLine {
	return tsoptions.ParseJsonConfigFileContent(json, &parseConfigHost{fs, basePath}, basePath, existingOptions, configFileName, resolutionStack, nil, cache)
}

func ParseJsonSourceFileConfigFileContent(source *tsoptions.TsConfigSourceFile, fs vfs.FS, basePath string, existingOptions *core.CompilerOptions, raw *collections.OrderedMap[string, any], resolutionStack []tspath.Path, cache tsoptions.ExtendedConfigCache) *tsoptions.ParsedCommandLine {
	return tsoptions.ParseJsonSourceFileConfigFileContent(source, &parseConfigHost{fs, basePath}, basePath, existingOptions, raw, source.SourceFile.FileName(), resolutionStack, nil, cache)
}

// NewParsedCommandLine adapts the explicit base directory and case sensitivity API.
func NewParsedCommandLine(options *core.CompilerOptions, files []string, references []*core.ProjectReference, directory string, sensitivity shimpath.CaseSensitivity) *tsoptions.ParsedCommandLine {
	config := tsoptions.NewParsedCommandLine(options, files, tspath.ComparePathsOptions{
		CurrentDirectory:          directory,
		UseCaseSensitiveFileNames: sensitivity == shimpath.CaseSensitive,
	})
	config.ParsedConfig.ProjectReferences = references
	return config
}
