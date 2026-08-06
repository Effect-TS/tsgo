// Package codegen defines the Codegen struct for Effect code generation directives.
package codegen

// Codegen defines a code generation directive with its metadata.
type Codegen struct {
	// Name is the unique identifier used in @effect-codegens directives.
	Name string

	// Description explains what the codegen does.
	Description string
}
