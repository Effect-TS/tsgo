import { Effect } from "effect"

const first = Effect.succeed(1)
const second = Effect.succeed("second")

export const program = first.pipe(Effect.flatMap(() => second))
