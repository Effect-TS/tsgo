package tsoptions

import (
	"github.com/microsoft/TypeScript/tsc/internal/collections"
	"github.com/microsoft/TypeScript/tsc/internal/core"
	"github.com/microsoft/TypeScript/tsc/internal/tsoptions"
	"github.com/microsoft/TypeScript/tsc/internal/tspath"
)

func ParseJsonSourceFileConfigFileContent(
	sourceFile *tsoptions.TsConfigSourceFile,
	host tsoptions.ParseConfigHost,
	basePath string,
	existingOptions *core.CompilerOptions,
	existingOptionsRaw *collections.OrderedMap[string, any],
	configFileName string,
	resolutionStack []tspath.Path,
	_ any,
	extendedConfigCache tsoptions.ExtendedConfigCache,
) *tsoptions.ParsedCommandLine {
	return tsoptions.ParseJsonSourceFileConfigFileContent(
		sourceFile,
		host,
		basePath,
		existingOptions,
		existingOptionsRaw,
		configFileName,
		resolutionStack,
		extendedConfigCache,
	)
}
