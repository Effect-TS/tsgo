package autoimport

import (
	"github.com/microsoft/typescript-go/internal/ls/autoimport"
)

func ModuleIDString(id autoimport.ModuleID) string { return string(id) }

func ModuleIDFileName(id autoimport.ModuleID) string { return string(id) }

func NewAmbientModuleID(specifier string) autoimport.ModuleID { return autoimport.ModuleID(specifier) }
