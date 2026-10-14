// @effect-diagnostics *:off
// @effect-diagnostics preferNamespaceImport:warning
// @test-config { "namespaceImportPackages": ["effect"] }
import { Effect } from "effect"

export const program = Effect.succeed(1)
