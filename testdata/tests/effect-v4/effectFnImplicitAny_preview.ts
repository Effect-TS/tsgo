// @effect-diagnostics *:off
// @effect-diagnostics effectFnImplicitAny:error
// @test-config {"diagnosticSeverity":{"effectFnImplicitAny":"error"}}
import { Effect } from "effect"

export const preview = Effect.fn("preview")((input) => Effect.succeed(input))
