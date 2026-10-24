// Package schemagen converts TypeScript AST type nodes into Effect Schema API
// call expressions. It is used by the typeToEffectSchema and typeToEffectSchemaClass
// refactors to generate Schema.Struct, Schema.Class, and related schema declarations
// from interface and type alias declarations.

mod schemagen;
mod structural;
pub use schemagen::*;
pub use structural::*;
