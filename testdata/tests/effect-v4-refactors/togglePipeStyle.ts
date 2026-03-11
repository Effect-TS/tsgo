// refactor: 5:54-5:55, 7:32-7:33
import * as Effect from "effect/Effect"
import { pipe } from "effect/Function"

export const toRegularPipe = Effect.succeed(42).pipe(Effect.map((x) => x * 2))

export const toPipeable = pipe(Effect.succeed(42), Effect.map((x) => x * 2))
