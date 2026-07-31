import { Effect } from "effect"

Effect.succeed("floating")
Effect.never
Effect.void
Effect.fail("error")

const assigned = Effect.succeed(1)
void assigned
