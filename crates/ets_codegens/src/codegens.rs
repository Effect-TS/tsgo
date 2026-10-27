// Package codegens contains all Effect codegen definitions and the registry.

use ets_codegen::Codegen;

// go: internal/codegens/codegens.go:9 All ab03647aed9f08ec
// All is the list of all codegens.
pub static ALL: &[Codegen] = &[
    Codegen { name: "accessors", description: "Generate accessors for the service" },
    Codegen { name: "annotate", description: "Annotate with type" },
    Codegen { name: "typeToSchema", description: "Generate Schemas from types" },
];
