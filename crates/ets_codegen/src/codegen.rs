// Package codegen defines the Codegen struct for Effect code generation directives.

// go: internal/codegen/codegen.go:5 Codegen
// Codegen defines a code generation directive with its metadata.
//
// Rust: codegens are package-level data in Go (`codegens.All`), so the strings are `'static`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Codegen {
    // Name is the unique identifier used in @effect-codegens directives.
    pub name: &'static str,

    // Description explains what the codegen does.
    pub description: &'static str,
}
