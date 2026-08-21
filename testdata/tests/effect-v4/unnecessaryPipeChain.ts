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

const bump = Effect.map((x: number) => x + 1)

// Should trigger: merging reaches the 20-transformation .pipe() limit
export const pipeableAtMergedLimit = Effect.succeed(0).pipe(
  bump, bump, bump, bump, bump,
  bump, bump, bump, bump, bump,
  bump, bump, bump, bump, bump,
  bump, bump, bump, bump
).pipe(bump)

// Should NOT trigger: merging exceeds the 20-transformation .pipe() limit
export const pipeableOverMergedLimit = Effect.succeed(0).pipe(
  bump, bump, bump, bump, bump,
  bump, bump, bump, bump, bump,
  bump, bump, bump, bump, bump,
  bump, bump, bump, bump, bump
).pipe(bump)

// Should trigger: merging reaches the 19-transformation pipe() limit
export const standaloneAtMergedLimit = pipe(
  pipe(
    Effect.succeed(0),
    bump, bump, bump, bump, bump,
    bump, bump, bump, bump
  ),
  bump, bump, bump, bump, bump,
  bump, bump, bump, bump, bump
)

// Should NOT trigger: merging exceeds the 19-transformation pipe() limit
export const standaloneOverMergedLimit = pipe(
  pipe(
    Effect.succeed(0),
    bump, bump, bump, bump, bump,
    bump, bump, bump, bump, bump
  ),
  bump, bump, bump, bump, bump,
  bump, bump, bump, bump, bump
)

// Should NOT trigger: the merged arity is unknown because of a spread argument
export const unknownMergedArity = Effect.succeed(0).pipe(
  ...([
    bump, bump, bump, bump, bump,
    bump, bump, bump, bump, bump,
    bump, bump, bump, bump, bump,
    bump, bump, bump, bump, bump
  ] as const)
).pipe(bump)

// Should still trigger for the safe inner merge when the outer merge exceeds the limit
export const nestedWithinSnoozedChain = Effect.succeed(0)
  .pipe(bump)
  .pipe(bump)
  .pipe(
    bump, bump, bump, bump, bump,
    bump, bump, bump, bump, bump,
    bump, bump, bump, bump, bump,
    bump, bump, bump, bump, bump
  )
