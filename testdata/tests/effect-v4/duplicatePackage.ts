import { Effect } from "effect"

// When only one version of effect is installed, no duplicatePackage warning should appear.
const program = Effect.gen(function* () {
  return yield* Effect.succeed(42)
})
