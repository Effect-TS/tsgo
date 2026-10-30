import { Context, Effect, Effect as Fx, Layer, ManagedRuntime, pipe } from "effect"
import { provideContext as provideContextNamed, runPromise as runPromiseNamed } from "effect/Effect"

interface Config {
  readonly apiUrl: string
}

const Config = Context.Service<Config>("Config")

const services = Context.make(Config, {
  apiUrl: "https://api.example.com"
})

const program = Effect.gen(function*() {
  const config = yield* Config
  return `Connecting to ${config.apiUrl}`
})

declare const signal: AbortSignal

// ---------------------------------------------------------------------------
// MUST TRIGGER - Effect.provideContext / Effect.setContext with a
// Context.Context is the outermost transformation handed to a non-*With runner.
// Every `fix:` below was applied verbatim and checked with tsc --strict against
// effect 4.0.2: each rewrite type-checks and has exactly the original's type.
// ---------------------------------------------------------------------------

// MUST TRIGGER - data-first provideContext into runPromise
// fix: Effect.runPromiseWith(services)(program)
export const dataFirstPromise = Effect.runPromise(Effect.provideContext(program, services))

// MUST TRIGGER - RunOptions of the original runner are carried over (runPromiseWith takes the same RunOptions)
// fix: Effect.runPromiseWith(services)(program, { signal })
export const dataFirstPromiseOptions = Effect.runPromise(Effect.provideContext(program, services), { signal })

// MUST TRIGGER - runPromiseExit -> runPromiseExitWith
// fix: Effect.runPromiseExitWith(services)(program)
export const dataFirstPromiseExit = Effect.runPromiseExit(Effect.provideContext(program, services))

// MUST TRIGGER - runSync -> runSyncWith
// fix: Effect.runSyncWith(services)(program)
export const dataFirstSync = Effect.runSync(Effect.provideContext(program, services))

// MUST TRIGGER - runSyncExit -> runSyncExitWith
// fix: Effect.runSyncExitWith(services)(program)
export const dataFirstSyncExit = Effect.runSyncExit(Effect.provideContext(program, services))

// MUST TRIGGER - runFork -> runForkWith, options carried over (runForkWith takes the same RunOptions)
// fix: Effect.runForkWith(services)(program, { uninterruptible: true })
export const dataFirstForkOptions = Effect.runFork(Effect.provideContext(program, services), { uninterruptible: true })

// MUST TRIGGER - runCallback -> runCallbackWith, the onExit options are carried over
// (same options type; `exit` is still contextually typed as Exit<string, never>)
// fix: Effect.runCallbackWith(services)(program, { onExit: (exit) => console.log(exit) })
export const dataFirstCallback = Effect.runCallback(Effect.provideContext(program, services), {
  onExit: (exit) => console.log(exit)
})

// MUST TRIGGER - method pipe whose only (and final) step is provideContext
// fix: Effect.runForkWith(services)(program)
export const methodPipeFork = Effect.runFork(program.pipe(Effect.provideContext(services)))

// MUST TRIGGER - provideContext is the final step after other transformations
// fix: Effect.runPromiseWith(services)(program.pipe(Effect.map((message) => message.length)))
export const methodPipeAfterMap = Effect.runPromise(program.pipe(
  Effect.map((message) => message.length),
  Effect.provideContext(services)
))

// MUST TRIGGER - function pipe whose final step is provideContext
// fix: Effect.runPromiseWith(services)(pipe(program, Effect.map((message) => message.length)))
export const functionPipeAfterMap = Effect.runPromise(pipe(
  program,
  Effect.map((message) => message.length),
  Effect.provideContext(services)
))

// MUST TRIGGER - data-last provideContext applied directly
// fix: Effect.runPromiseWith(services)(program)
export const dataLastApplied = Effect.runPromise(Effect.provideContext(services)(program))

// MUST TRIGGER - parentheses around the runner's effect argument
// fix: Effect.runPromiseWith(services)(program)
export const parenthesized = Effect.runPromise((Effect.provideContext(program, services)))

// MUST TRIGGER - setContext data-first
// fix: Effect.runPromiseWith(services)(program)
export const setContextDataFirst = Effect.runPromise(Effect.setContext(program, services))

// MUST TRIGGER - setContext as the final method pipe step
// fix: Effect.runSyncWith(services)(program)
export const setContextMethodPipe = Effect.runSync(program.pipe(Effect.setContext(services)))

// MUST TRIGGER - the runner itself is the next pipe step after provideContext
// fix: program.pipe(Effect.runPromiseWith(services))
export const runnerAsMethodPipeStep = program.pipe(Effect.provideContext(services), Effect.runPromise)

// MUST TRIGGER - the runner itself is the next function pipe step after provideContext
// fix: pipe(program, Effect.map((message) => message.length), Effect.runForkWith(services))
export const runnerAsFunctionPipeStep = pipe(
  program,
  Effect.map((message) => message.length),
  Effect.provideContext(services),
  Effect.runFork
)

// MUST TRIGGER - the context is an inline Context.make call
// fix: Effect.runPromiseWith(Context.make(Config, { apiUrl: "inline" }))(program)
export const inlineContext = Effect.runPromise(Effect.provideContext(program, Context.make(Config, { apiUrl: "inline" })))

// MUST TRIGGER - context built with Context.add on top of another context
// fix: Effect.runPromiseWith(Context.add(Context.empty(), Config, { apiUrl: "added" }))(program)
export const addedContext = Effect.runPromise(
  Effect.provideContext(program, Context.add(Context.empty(), Config, { apiUrl: "added" }))
)

// MUST TRIGGER - a context taken as a parameter (shape of Stream.toReadableStreamWith)
// fix: Effect.runForkWith(context)(effect)
export const forkWithParameter = (effect: Effect.Effect<void, never, Config>, context: Context.Context<Config>) =>
  Effect.runFork(Effect.provideContext(effect, context))

// MUST TRIGGER - generic context parameter, exactly as in Stream.toReadableStreamWith
// (both forms are Fiber<void, never>)
// fix: Effect.runForkWith(context)(effect)
export const forkWithGenericContext = <XR>(effect: Effect.Effect<void, never, XR>, context: Context.Context<XR>) =>
  Effect.runFork(Effect.provideContext(effect, context))

// MUST TRIGGER - the context provides more services than the effect needs; runPromiseWith
// infers R = Config | "Other" and accepts the narrower Effect<string, never, Config>
// fix: Effect.runPromiseWith(widerContext)(program)
declare const widerContext: Context.Context<Config | "Other">
export const widerProvided = Effect.runPromise(Effect.provideContext(program, widerContext))

// MUST TRIGGER - namespace alias
// fix: Fx.runPromiseWith(services)(program)
export const namespaceAlias = Fx.runPromise(Fx.provideContext(program, services))

// MUST TRIGGER - aliased runner still resolves to Effect.runPromise
// fix: Effect.runPromiseWith(services)(program)
const runPromise = Effect.runPromise
export const aliasedRunner = runPromise(Effect.provideContext(program, services))

// MUST TRIGGER - named imports from effect/Effect resolve to the same APIs; the fix uses
// the file's Effect module reference
// fix: Effect.runPromiseWith(services)(program)
export const namedImports = runPromiseNamed(provideContextNamed(program, services))

// MUST TRIGGER - the Effect import is shadowed by the parameter, so the fix reaches
// Effect.runPromiseWith through the other value import of the Effect module, Fx
// fix: Fx.runPromiseWith(services)(program)
export const shadowedEffect = (Effect: string) => [Effect, runPromiseNamed(provideContextNamed(program, services))] as const

// ---------------------------------------------------------------------------
// MUST TRIGGER - evaluation order. A called runner evaluates the effect first and
// the context last, while run*With(context)(effect) evaluates the context first,
// so the fix is withheld unless the context cannot observe or affect that work.
// ---------------------------------------------------------------------------

let currentServices = services
const switchServices = () => {
  currentServices = Context.make(Config, { apiUrl: "https://switched.example.com" })
  return program
}

// MUST TRIGGER - no fix (report only): run*With(context)(effect) would read the
// reassignable `currentServices` before `switchServices()` reassigns it (the original
// runs with the switched context, the rewrite with the old one)
export const reassignedContext = Effect.runSync(Effect.provideContext(switchServices(), currentServices))

// MUST TRIGGER - no fix (report only): a context computed by a call would move ahead
// of the effect expression and its pipe steps
declare const loadServices: () => Context.Context<Config>
export const computedContext = Effect.runPromise(program.pipe(Effect.provideContext(loadServices())))

// MUST TRIGGER - no fix (report only): a property read may change while the effect
// expression is evaluated
declare const holder: { services: Context.Context<Config> }
export const propertyContext = Effect.runPromise(Effect.provideContext(program, holder.services))

// MUST TRIGGER - the curried form already evaluates the context first, so the fix keeps the order
// fix: Effect.runPromiseWith(loadServices())(program)
export const curriedComputedContext = Effect.runPromise(Effect.provideContext(loadServices())(program))

// MUST TRIGGER - a runner pipe step keeps the source order, so the fix is offered
// fix: program.pipe(Effect.runPromiseWith(loadServices()))
export const runnerStepComputedContext = program.pipe(Effect.provideContext(loadServices()), Effect.runPromise)

// A const binding is only a stable reference: Context.makeUnsafe shares a Map
// that later code can update. Constructors read their Context arguments when
// they are evaluated, the provided Context itself is read when the effect runs.
const backing = new Map<string, unknown>([[Config.key, { apiUrl: "https://initial.example.com" }]])
const unsafeBase = Context.makeUnsafe<Config>(backing)
const refreshServices = () => {
  backing.set(Config.key, { apiUrl: "https://refreshed.example.com" })
  return program
}

// MUST TRIGGER - no fix (report only): Context.mergeAll snapshots unsafeBase, so the
// rewrite would snapshot it before refreshServices() updates the backing map
export const mergedFromBinding = Effect.runSync(Effect.provideContext(refreshServices(), Context.mergeAll(unsafeBase)))

// MUST TRIGGER - no fix (report only): Context.add copies an existing context when evaluated
export const addedToBinding = Effect.runSync(
  Effect.provideContext(refreshServices(), Context.add(unsafeBase, Config, { apiUrl: "added" }))
)

// MUST TRIGGER - a bare reference is read when the effect runs in both forms, after
// refreshServices() has updated the backing map
// fix: Effect.runSyncWith(unsafeBase)(refreshServices())
export const bareUnsafeReference = Effect.runSync(Effect.provideContext(refreshServices(), unsafeBase))

// MUST TRIGGER - constructors over fresh contexts only read what they create
// fix: Effect.runSyncWith(Context.merge(Context.empty(), Context.make(Config, { apiUrl: "merged" })))(refreshServices())
export const mergedFresh = Effect.runSync(
  Effect.provideContext(refreshServices(), Context.merge(Context.empty(), Context.make(Config, { apiUrl: "merged" })))
)

// ---------------------------------------------------------------------------
// MUST NOT TRIGGER - already using the dedicated runners
// ---------------------------------------------------------------------------

// MUST NOT TRIGGER - already the *With form
export const alreadyPromiseWith = Effect.runPromiseWith(services)(program)
export const alreadyForkWith = Effect.runForkWith(services)(program)
export const alreadyPromiseWithPipe = program.pipe(Effect.runPromiseWith(services))

// MUST NOT TRIGGER - a *With runner is not one of the six table entries, even when
// it receives a provided effect (merging contexts is a different rewrite)
const extraServices = Context.make(Config, { apiUrl: "https://other.example.com" })
export const withRunnerOfProvided = Effect.runPromiseWith(services)(Effect.provideContext(program, extraServices))

// MUST NOT TRIGGER - ordinary run of an effect without requirements
export const ordinaryRun = Effect.runPromise(Effect.succeed(1))

// ---------------------------------------------------------------------------
// MUST NOT TRIGGER - provideContext is not the outermost transformation
// ---------------------------------------------------------------------------

// MUST NOT TRIGGER - another pipe step follows provideContext
export const transformedAfterProvide = Effect.runPromise(program.pipe(
  Effect.provideContext(services),
  Effect.map((message) => message.length)
))

// MUST NOT TRIGGER - another function pipe step follows provideContext
export const functionPipeTransformedAfterProvide = Effect.runPromise(pipe(
  program,
  Effect.provideContext(services),
  Effect.asVoid
))

// MUST NOT TRIGGER - data-first provideContext nested inside another transformation
export const nestedDataFirst = Effect.runPromise(
  Effect.map(Effect.provideContext(program, services), (message) => message.length)
)

// MUST NOT TRIGGER - the runner is not the step right after provideContext
export const runnerNotAdjacent = program.pipe(Effect.provideContext(services), Effect.asVoid, Effect.runPromise)

// MUST NOT TRIGGER - the provided effect is stored first and run by reference
const providedProgram = Effect.provideContext(program, services)
export const referencedProvided = Effect.runPromise(providedProgram)

// MUST NOT TRIGGER - provideContext not handed to any runner
export const notRun = Effect.provideContext(program, services)
export const forkedChild = Effect.forkChild(Effect.provideContext(program, services))

// ---------------------------------------------------------------------------
// MUST NOT TRIGGER - other ways of providing services
// ---------------------------------------------------------------------------

// MUST NOT TRIGGER - Effect.provide (even with a Context) is not provideContext/setContext
export const provideWithContext = Effect.runPromise(Effect.provide(program, services))

// MUST NOT TRIGGER - Effect.provide with a Layer
const ConfigLive = Layer.succeed(Config, { apiUrl: "https://layer.example.com" })
export const provideWithLayer = Effect.runPromise(Effect.provide(program, ConfigLive))

// MUST NOT TRIGGER - Effect.provideService with a single service implementation
export const provideService = Effect.runPromise(Effect.provideService(program, Config, { apiUrl: "service" }))

// MUST NOT TRIGGER - Effect.updateContext derives the context from the current one
export const updateContext = Effect.runPromise(
  Effect.updateContext(program, (current: Context.Context<never>) => Context.merge(current, services))
)

// MUST NOT TRIGGER - the provided argument is not typed as a Context.Context
declare const untypedContext: any
export const anyContext = Effect.runPromise(Effect.provideContext(program, untypedContext))

// ---------------------------------------------------------------------------
// MUST NOT TRIGGER - local helpers and look-alike APIs
// ---------------------------------------------------------------------------

// MUST NOT TRIGGER - a local helper that wraps provideContext (ManagedRuntime-like
// provide that flatMaps a context effect before providing)
declare const contextEffect: Effect.Effect<Context.Context<Config>>
const provide = <A, E>(effect: Effect.Effect<A, E, Config>) =>
  Effect.flatMap(contextEffect, (context) => Effect.provideContext(effect, context))
export const localProvideHelper = Effect.runPromise(provide(program))

// MUST NOT TRIGGER - a local helper that directly returns provideContext
const withServices = <A, E>(effect: Effect.Effect<A, E, Config>) => Effect.provideContext(effect, services)
export const localDirectHelper = Effect.runPromise(withServices(program))

// MUST NOT TRIGGER - a local runner helper is not Effect.runFork
const runForkLocal = <A, E>(effect: Effect.Effect<A, E>) => Effect.runFork(effect)
export const localRunnerHelper = runForkLocal(Effect.provideContext(program, services))

// MUST NOT TRIGGER - ManagedRuntime runners are not Effect runners
const runtime = ManagedRuntime.make(Layer.empty)
export const managedRuntimeRun = runtime.runPromise(Effect.provideContext(program, services))

// MUST NOT TRIGGER - look-alike APIs with the same names
const LocalEffect = {
  provideContext: <A, E>(effect: Effect.Effect<A, E, Config>, context: Context.Context<Config>) =>
    Effect.provideContext(effect, context),
  runPromise: <A, E>(effect: Effect.Effect<A, E>) => Effect.runPromise(effect)
}
export const localProvideContext = Effect.runPromise(LocalEffect.provideContext(program, services))
export const localRunPromise = LocalEffect.runPromise(Effect.provideContext(program, services))
