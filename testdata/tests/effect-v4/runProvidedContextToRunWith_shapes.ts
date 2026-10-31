import { Context, Effect, pipe } from "effect"

declare const program: Effect.Effect<string, never, "Config">
declare const services: Context.Context<"Config">
declare const load: () => Context.Context<"Config">

// ---------------------------------------------------------------------------
// MUST TRIGGER - call shapes covered by the same piping-flow match
// ---------------------------------------------------------------------------

// MUST TRIGGER - curried setContext inside redundant parentheses
// fix: Effect.runSyncWith(services)(program)
export const curriedParenthesized = Effect.runSync((Effect.setContext(services)(program)))

// MUST TRIGGER - setContext and runSyncExit as Function.pipe steps
// fix: pipe(program, Effect.runSyncExitWith(services))
export const functionPipeRunner = pipe(program, Effect.setContext(services), Effect.runSyncExit)

// MUST TRIGGER - runCallback as the method pipe step after provideContext
// fix: program.pipe(Effect.runCallbackWith(services))
export const methodPipeRunner = program.pipe(Effect.provideContext(services), Effect.runCallback)

// MUST TRIGGER - the run is a data-first value argument of another call
// fix: Effect.as(Effect.void, Effect.runSyncWith(services)(program))
export const valueArgument = Effect.as(Effect.void, Effect.runSync(Effect.provideContext(program, services)))

// MUST TRIGGER - provideContext and the runner as Effect.fn pipeable steps
// fix: Effect.fn(function*() { return yield* program }, Effect.runPromiseWith(services))
export const fnSteps = Effect.fn(function*() {
  return yield* program
}, Effect.provideContext(services), Effect.runPromise)

// MUST TRIGGER - the same steps on Effect.fnUntraced
// fix: Effect.fnUntraced(function*() { return yield* program }, Effect.runSyncWith(services))
export const fnUntracedSteps = Effect.fnUntraced(function*() {
  return yield* program
}, Effect.setContext(services), Effect.runSync)

// MUST TRIGGER - constructors over fresh contexts are evaluated in either order
// fix: Effect.runPromiseWith(Context.merge(Context.empty(), Context.empty()))(Effect.succeed(1))
export const freshConstructor = Effect.runPromise(
  Effect.provideContext(Effect.succeed(1), Context.merge(Context.empty(), Context.empty()))
)

// MUST TRIGGER - no fix (report only): Context.merge reads the bound `services`
// when it is evaluated, which the rewrite would move ahead of the effect
export const constructorOverBinding = Effect.runPromise(
  Effect.provideContext(program, Context.merge(services, Context.empty()))
)

// MUST TRIGGER - no fix (report only): the computed context of a called runner would
// move ahead of the effect, even when the runner has options
declare const signal: AbortSignal
export const computedWithOptions = Effect.runFork(Effect.provideContext(program, load()), { signal })

// MUST TRIGGER - no fix (report only): explicit runner type arguments
export const runnerTypeArguments = Effect.runPromise<string, never>(Effect.provideContext(program, services))

// ---------------------------------------------------------------------------
// MUST NOT TRIGGER - near misses
// ---------------------------------------------------------------------------

// MUST NOT TRIGGER - an Effect.fn step follows provideContext before the runner
export const fnNotAdjacent = Effect.fn(function*() {
  return yield* program
}, Effect.provideContext(services), Effect.asVoid, Effect.runPromise)

// MUST NOT TRIGGER - the provided effect is a data-first subject that no runner takes
export const providedAsValue = Effect.as(Effect.provideContext(program, services), 1)
