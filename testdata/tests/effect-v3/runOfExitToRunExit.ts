import { Effect, pipe } from "effect"

declare const program: Effect.Effect<number, Error>
declare const signal: AbortSignal

export const directPromise = Effect.runPromise(Effect.exit(program), { signal })
export const methodPipe = Effect.runPromise(program.pipe(Effect.exit))
export const functionPipe = Effect.runPromise(pipe(program, Effect.exit))
export const sameFunctionPipe = pipe(program, Effect.exit, Effect.runPromise)

export const transformedExit = Effect.runPromise(
  program.pipe(Effect.exit, Effect.map((exit) => exit))
)
const computedExit = Effect.exit(program)
export const referencedExit = Effect.runPromise(computedExit)

// Not equivalent when the input performs asynchronous work.
export const directSync = Effect.runSync(Effect.exit(program))
export const functionPipeSync = Effect.runSync(pipe(program, Effect.exit))
