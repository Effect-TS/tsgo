package tsoptions

import (
	"github.com/microsoft/typescript-go/internal/collections"
	"github.com/microsoft/typescript-go/internal/core"
	"github.com/microsoft/typescript-go/internal/tsoptions"
	"github.com/microsoft/typescript-go/internal/tspath"
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
