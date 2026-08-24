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
	extraFileExtensions any,
	extendedConfigCache tsoptions.ExtendedConfigCache,
) *tsoptions.ParsedCommandLine {
	var extensions []tsoptions.FileExtensionInfo
	if extraFileExtensions != nil {
		extensions = extraFileExtensions.([]tsoptions.FileExtensionInfo)
	}
	return tsoptions.ParseJsonSourceFileConfigFileContent(
		sourceFile,
		host,
		basePath,
		existingOptions,
		existingOptionsRaw,
		configFileName,
		resolutionStack,
		extensions,
		extendedConfigCache,
	)
}
