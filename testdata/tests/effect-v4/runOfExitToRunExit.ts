import { Context, Effect, Effect as Fx, pipe } from "effect"

declare const program: Effect.Effect<number, Error>
declare const context: Context.Context<never>
declare const signal: AbortSignal

// Direct calls, including RunOptions.
export const directPromise = Effect.runPromise(Effect.exit(program))
export const directPromiseOptions = Effect.runPromise(Effect.exit(program), { signal })

// The final transformation of the runner's effect argument is Effect.exit.
export const methodPipeArgument = Effect.runPromise(program.pipe(
  Effect.map((value) => value + 1),
  Effect.exit
))
export const functionPipeArgument = Effect.runPromise(pipe(
  program,
  Effect.map((value) => value + 1),
  Effect.exit
))

// The runner itself may also be used as a pipe transformation.
export const sameMethodPipe = program.pipe(Effect.exit, Effect.runPromise)
export const sameFunctionPipe = pipe(program, Effect.exit, Effect.runPromise)

// Effect v4 context-bound runners have the same fusion.
export const promiseWith = Effect.runPromiseWith(context)(Effect.exit(program), { signal })
export const promiseWithPipe = program.pipe(Effect.exit, Effect.runPromiseWith(context))

// Namespace aliases remain fixable.
export const alias = Fx.runPromise((Fx.exit(program)))

// Effect.exit is not the runner input in these cases.
export const transformedExit = Effect.runPromise(
  program.pipe(Effect.exit, Effect.map((exit) => exit))
)
const computedExit = Effect.exit(program)
export const referencedExit = Effect.runPromise(computedExit)

// Dedicated runners and ordinary runs are already good.
export const alreadyPromiseExit = Effect.runPromiseExit(program)
export const alreadySyncExit = Effect.runSyncExit(program)
export const ordinaryRun = Effect.runPromise(program)

// The sync rewrite is not equivalent for asynchronous effects: runSync throws
// AsyncFiberError before Effect.exit completes, while runSyncExit returns it in
// an Exit.Failure. Without a static synchrony guarantee these must not trigger.
export const directSync = Effect.runSync(Effect.exit(program))
export const sameFunctionPipeSync = pipe(program, Effect.exit, Effect.runSync)
export const syncWith = Effect.runSyncWith(context)(program.pipe(Effect.exit))

// Symbol verification excludes look-alike APIs.
const LocalEffect = {
  exit: <A, E>(effect: Effect.Effect<A, E>) => Effect.exit(effect),
  runPromise: <A>(effect: Effect.Effect<A>) => Effect.runPromise(effect)
}
export const localExit = Effect.runPromise(LocalEffect.exit(Effect.succeed(1)))
export const localRunner = LocalEffect.runPromise(Effect.exit(Effect.succeed(1)))
