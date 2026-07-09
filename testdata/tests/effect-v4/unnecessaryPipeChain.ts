import * as Effect from "effect/Effect"
import { pipe } from "effect/Function"

// Should trigger: chained .pipe() calls
export const asPipeable = Effect.succeed(1).pipe(Effect.map((x) => x + 2)).pipe(Effect.map((x) => x + 3))

// Should trigger: nested pipe() calls
export const asPipe = pipe(pipe(Effect.succeed(1), Effect.map((x) => x + 2)), Effect.map((x) => x + 3))

// Should NOT trigger: single pipe with multiple args
export const singlePipe = pipe(Effect.succeed(1), Effect.map((x) => x + 2), Effect.map((x) => x + 3))

// Should NOT trigger: single .pipe() with multiple args
export const singlePipeable = Effect.succeed(1).pipe(Effect.map((x) => x + 2), Effect.map((x) => x + 3))

// Should NOT trigger: pipe with non-pipe subject
export const nonPipeSubject = pipe(Effect.succeed(1), Effect.map((x) => x + 2))
