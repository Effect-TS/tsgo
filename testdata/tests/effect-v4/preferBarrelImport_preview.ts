// @effect-diagnostics *:off
// @effect-diagnostics preferBarrelImport:warning
// @test-config { "barrelImportPackages": ["effect"] }
import * as Record from "effect/Record"

export const keys = Record.keys({ a: 1 })
