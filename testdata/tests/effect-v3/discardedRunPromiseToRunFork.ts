// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics discardedRunPromiseToRunFork:warning
import { Cause, Deferred, Effect, Effect as Fx, Layer, ManagedRuntime, Runtime } from "effect"
import * as Eff from "effect/Effect"
import { runPromise } from "effect/Effect"

declare class EmitError extends Error {
  readonly _tag: "EmitError"
}
interface Port {
  postMessage(message: unknown): void
}
declare const port: Port
declare const reply: (port: Port, message: unknown) => Effect.Effect<void, EmitError>
declare const work: Effect.Effect<void, Error>
declare const program: Effect.Effect<number, Error>
declare const services: Runtime.Runtime<never>
declare const signal: AbortSignal
declare const seen: Deferred.Deferred<void>
declare const onStatement: (cb: (sql: string) => void) => void
declare const flag: boolean
declare const consume: (promise: Promise<unknown>) => void

// MUST TRIGGER: bare expression statement, the returned Promise floats
Effect.runPromise(program)

// MUST TRIGGER: `void` makes the discard explicit, the rejection is still unhandled
void Effect.runPromise(program)

// MUST TRIGGER: a typed failure discarded at an entry point
void Effect.runPromise(reply(port, { kind: "pong" }))

// MUST TRIGGER: the recovery path is itself fallible
void Effect.runPromise(
  work.pipe(Effect.catchAllCause((cause) => reply(port, { kind: "error", detail: Cause.pretty(cause) })))
)

// MUST TRIGGER: the error channel is `never` but defects still reject the Promise
void Effect.runPromise(
  Effect.matchCauseEffect(work, {
    onFailure: (cause) => Effect.sync(() => port.postMessage({ kind: "error", detail: Cause.pretty(cause) })),
    onSuccess: () => Effect.sync(() => port.postMessage({ kind: "done" }))
  })
)

// MUST TRIGGER: a synchronous callback with nowhere to await
onStatement((sql) => {
  if (sql.includes("DELETE")) {
    void Effect.runPromise(Deferred.succeed(seen, void 0))
  }
})

// MUST TRIGGER: with options (no quick fix: v3 runFork options do not accept `signal`)
Effect.runPromise(program, { signal })

// MUST TRIGGER: parentheses around the discarded call are transparent
;(Effect.runPromise(program))
void (Effect.runPromise(program))

// MUST TRIGGER: non-final operand of a comma expression
export const commaFirst = (Effect.runPromise(program), 1)

// MUST TRIGGER: final operand of a comma expression that is itself a discarded statement
port.postMessage("starting"), Effect.runPromise(program)

// MUST TRIGGER: statement inside a block-bodied arrow function
export const blockArrow = () => {
  Effect.runPromise(program)
}

// MUST TRIGGER: `void` inside a concise arrow body, the arrow returns undefined, not the Promise
export const voidArrow = () => void Effect.runPromise(program)

// MUST TRIGGER: an async function does not await the Promise just by being async
export async function asyncMain() {
  Effect.runPromise(program)
}

// MUST TRIGGER: `return void` still discards the Promise
export function returnVoid(): void {
  return void Effect.runPromise(program)
}

// MUST TRIGGER: class method body
export class Server {
  start() {
    Effect.runPromise(program)
  }
}

// MUST TRIGGER: Effect namespace alias
Fx.runPromise(program)

// MUST TRIGGER: star import of effect/Effect
Eff.runPromise(program)

// MUST TRIGGER: named import of runPromise (no quick fix: a bare identifier cannot be renamed in place)
runPromise(program)

// MUST TRIGGER: local alias of Effect.runPromise (no quick fix)
const runPromiseAlias = Effect.runPromise
runPromiseAlias(program)

// MUST TRIGGER: Runtime.runPromise(runtime)(...), the v3 counterpart of Effect.runPromiseWith
Runtime.runPromise(services)(work)
void Runtime.runPromise(services, work)

// MUST TRIGGER: hoisted Runtime.runPromise(runtime) runner, the call site is a bare identifier (no quick fix)
const run = Runtime.runPromise(services)
void run(work)

// MUST TRIGGER: ManagedRuntime receiver, the fix renames the member to runFork
const runtime = ManagedRuntime.make(Layer.empty)
void runtime.runPromise(work)

// MUST NOT TRIGGER: awaited
export async function awaited() {
  await Effect.runPromise(program)
  await Runtime.runPromise(services)(work)
  await runtime.runPromise(work)
}

// MUST NOT TRIGGER: returned from an arrow body, the caller owns the Promise
export const handler = (e: Effect.Effect<void, Error>) => Effect.runPromise(e)

// MUST NOT TRIGGER: returned from a block body
export function returned() {
  return Effect.runPromise(program)
}

// MUST NOT TRIGGER: assigned
export const assigned = Effect.runPromise(program)
let reassigned: Promise<number>
reassigned = Effect.runPromise(program)

// MUST NOT TRIGGER: passed as an argument
export const all = Promise.all([Effect.runPromise(program)])
consume(Effect.runPromise(program))

// MUST NOT TRIGGER: the rejection is consumed by .catch / .then / .finally
void Effect.runPromise(program).catch(() => {})
Effect.runPromise(program).then(() => {}, () => {})
Effect.runPromise(program).finally(() => {})
void Runtime.runPromise(services)(work).catch(() => {})
void run(work).catch(() => {})
void runtime.runPromise(work).catch(() => {})

// MUST NOT TRIGGER: final operand of a comma expression whose value is used
export const commaLast = (port.postMessage("starting"), Effect.runPromise(program))

// MUST NOT TRIGGER: conditional and logical operands are outside the first iteration's discard positions
flag ? Effect.runPromise(program) : undefined
flag && Effect.runPromise(program)

// MUST NOT TRIGGER: Exit runners never reject, a discarded Exit Promise is a different problem
Effect.runPromiseExit(program)
void Runtime.runPromiseExit(services)(work)
void runtime.runPromiseExit(work)

// MUST NOT TRIGGER: already fire-and-forget through a Fiber
Effect.runFork(program)
Runtime.runFork(services)(work)
runtime.runFork(work)

// MUST NOT TRIGGER: other runners
Effect.runCallback(program)
void Effect.runSync(Effect.succeed(1))

// MUST NOT TRIGGER: referencing the runner without calling it
void Effect.runPromise

// MUST NOT TRIGGER: the curried Runtime.runPromise(runtime) call itself returns a function, not a Promise
void Runtime.runPromise(services)

// MUST NOT TRIGGER: look-alike APIs that are not Effect's runners
const LocalEffect = {
  runPromise: <A>(effect: Effect.Effect<A>) => Effect.runPromise(effect)
}
LocalEffect.runPromise(Effect.succeed(1))
const localRuntime = {
  runPromise: (_: Effect.Effect<void, Error>) => Promise.resolve()
}
void localRuntime.runPromise(work)
declare const runPromiseLike: (effect: Effect.Effect<void, Error>) => Promise<void>
void runPromiseLike(work)
