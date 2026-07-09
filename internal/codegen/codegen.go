// Package codegen defines the Codegen struct for Effect code generation directives.
package codegen

// Codegen defines a code generation directive with its metadata.
type Codegen struct {
	// Name is the unique identifier used in @effect-codegens directives.
	Name string

	// Description explains what the codegen does.
	Description string
}

// ByName finds a codegen by name in a slice. Returns nil if not found.
func ByName(codegens []Codegen, name string) *Codegen {
	for i := range codegens {
		if codegens[i].Name == name {
			return &codegens[i]
		}
	}
	return nil
}
