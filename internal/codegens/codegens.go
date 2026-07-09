// Package codegens contains all Effect codegen definitions and the registry.
package codegens

import (
	"github.com/effect-ts/tsgo/internal/codegen"
)

// All is the list of all codegens.
var All = []codegen.Codegen{
	{
		Name:        "accessors",
		Description: "Generate accessors for the service",
	},
	{
		Name:        "annotate",
		Description: "Annotate with type",
	},
	{
		Name:        "typeToSchema",
		Description: "Generate Schemas from types",
	},
}

// ByName finds a codegen by name. Returns nil if not found.
func ByName(name string) *codegen.Codegen {
	return codegen.ByName(All, name)
}
