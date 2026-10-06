package autoimport

import (
	"github.com/microsoft/typescript-go/internal/ls/autoimport"
	"github.com/microsoft/typescript-go/internal/tspath"
	_ "unsafe"
)

func ModuleIDString(id autoimport.ModuleID) string { return id.AsString() }

func ModuleIDFileName(id autoimport.ModuleID) tspath.RootedFilePath {
	path, ok := id.AsPathKey()
	if !ok {
		return ""
	}
	return tspath.RootedFilePathFromNormalized(string(path))
}

//go:linkname ambientModuleID github.com/microsoft/TypeScript/tsc/internal/ls/autoimport.ambientModuleID
func ambientModuleID(string) autoimport.ModuleID

func NewAmbientModuleID(specifier string) autoimport.ModuleID {
	if specifier == "" {
		return autoimport.ModuleID{}
	}
	return ambientModuleID(specifier)
}
