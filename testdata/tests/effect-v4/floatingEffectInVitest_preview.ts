// @effect-diagnostics *:off
// @effect-diagnostics floatingEffectInVitest:error
import { it } from "@effect/vitest"
import { Effect } from "effect"

it("does not execute", () => Effect.void)
