package tsoptions

import (
	"github.com/microsoft/typescript-go/shim/collections"
	"github.com/microsoft/typescript-go/shim/core"
	"github.com/microsoft/typescript-go/shim/tsoptions"
	"github.com/microsoft/typescript-go/shim/tspath"
)

func ParseJsonConfigFileContent(
	json any,
	host tsoptions.ParseConfigHost,
	basePath string,
	existingOptions *core.CompilerOptions,
	configFileName string,
	resolutionStack []tspath.Path,
	extraFileExtensions any,
	extendedConfigCache tsoptions.ExtendedConfigCache,
) *tsoptions.ParsedCommandLine {
	var extensions []tsoptions.FileExtensionInfo
	if extraFileExtensions != nil {
		extensions = extraFileExtensions.([]tsoptions.FileExtensionInfo)
	}
	return tsoptions.ParseJsonConfigFileContent(
		json,
		host,
		basePath,
		existingOptions,
		configFileName,
		resolutionStack,
		extensions,
		extendedConfigCache,
	)
}

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
