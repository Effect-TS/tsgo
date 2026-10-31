// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics discardedRunPromiseToRunFork:warning
import { Effect, ManagedRuntime, Runtime } from "effect"

declare const work: Effect.Effect<void, Error>
declare const services: Runtime.Runtime<never>

// The fix renames the runner in place. That is only sound on a receiver known
// to own the replacement with its declared signature: the Effect or Runtime
// module itself, or a plain ManagedRuntime<R, ER> instantiation. Every other
// receiver keeps the diagnostic without a fix.

type MR<R = never> = ManagedRuntime.ManagedRuntime<R, never>
interface Service {
  readonly name: string
}
type Nullable<T> = { [K in keyof T]: T[K] | undefined }
type Stringified<T> = { [K in keyof T]: string }
type Thunked<T> = { [K in keyof T]: () => void }
type Identity<T> = { [K in keyof T]: T[K] }

// MUST TRIGGER (no quick fix): the receiver is narrowed to runPromise only
declare const promiseOnly: Pick<MR, "runPromise">
void promiseOnly.runPromise(work)

// MUST TRIGGER (no quick fix): runFork is omitted from the receiver
declare const withoutFork: Omit<MR, "runFork">
void withoutFork.runPromise(work)

// MUST TRIGGER (no quick fix): an interface inheriting only runPromise
interface PromiseRunner extends Pick<MR, "runPromise"> {}
declare const promiseRunner: PromiseRunner
void promiseRunner.runPromise(work)

// MUST TRIGGER (no quick fix): a narrowed receiver that keeps runFork is still a mapped type
declare const withFork: Pick<MR, "runPromise" | "runFork">
void withFork.runPromise(work)

// MUST TRIGGER (no quick fix): runPromise and runFork come from different runtimes,
// so runFork would run the effect without the Service runtime
declare const mixedRuntimes: Pick<MR<Service>, "runPromise"> & Pick<MR<never>, "runFork">
void mixedRuntimes.runPromise(work)

// MUST TRIGGER (no quick fix): the receiver's runFork is not Effect's runner
declare const shadowedFork: Omit<MR, "runFork"> & {
  readonly runFork: (effect: Effect.Effect<void, Error>) => string
}
void shadowedFork.runPromise(work)

// MUST TRIGGER (no quick fix): a ManagedRuntime intersected with another runFork
declare const mixedFork: MR & {
  readonly runFork: (effect: Effect.Effect<void, Error>) => string
}
void mixedFork.runPromise(work)

// MUST TRIGGER (no quick fix): runFork is optional
declare const optionalFork: Pick<MR, "runPromise"> & Partial<Pick<MR, "runFork">>
void optionalFork.runPromise(work)

// MUST TRIGGER (no quick fix): runFork may be undefined
declare const nullableFork: Pick<MR, "runPromise"> & Nullable<Pick<MR, "runFork">>
void nullableFork.runPromise(work)

// MUST TRIGGER (no quick fix): runFork is mapped to a non-callable type
declare const stringFork: Pick<MR, "runPromise"> & Stringified<Pick<MR, "runFork">>
void stringFork.runPromise(work)

// MUST TRIGGER (no quick fix): runFork is mapped to another signature
declare const thunkFork: Pick<MR, "runPromise"> & Thunked<Pick<MR, "runFork">>
void thunkFork.runPromise(work)

// MUST TRIGGER (no quick fix): a readonly copy is a mapped type, not a plain ManagedRuntime
declare const readonlyRuntime: Readonly<MR>
void readonlyRuntime.runPromise(work)

// MUST TRIGGER (no quick fix): an identity mapped copy is not a plain ManagedRuntime
declare const identityRuntime: Identity<MR>
void identityRuntime.runPromise(work)

// MUST TRIGGER (no quick fix): an interface extending ManagedRuntime is not a plain ManagedRuntime
interface AppRuntime extends MR {}
declare const appRuntime: AppRuntime
void appRuntime.runPromise(work)

// MUST TRIGGER (no quick fix): a generic receiver is a type parameter, not a plain ManagedRuntime
export const generic = <R extends MR>(runtime: R) => {
  void runtime.runPromise(work)
}

// MUST TRIGGER (no quick fix): a union of runtimes is not a single ManagedRuntime
declare const eitherRuntime: MR | MR<Service>
void eitherRuntime.runPromise(work)

// MUST TRIGGER (no quick fix): the returned runner is still declared by Runtime.runPromise,
// but the receiver is a narrowed copy of the Runtime module, not the module itself
declare const promiseRunners: Pick<typeof Runtime, "runPromise">
void promiseRunners.runPromise(services)(work)

// MUST NOT TRIGGER: a mapped copy of a module export is a new symbol, not Effect.runPromise
// (runners are resolved by symbol, as for the look-alike APIs)
declare const effectPromiseRunner: Pick<typeof Effect, "runPromise">
void effectPromiseRunner.runPromise(work)

// MUST TRIGGER (with quick fix): the Runtime module itself, parenthesized
void (Runtime).runPromise(services)(work)

// MUST TRIGGER (with quick fix): a plain ManagedRuntime, through a type alias
declare const plainRuntime: MR
void plainRuntime.runPromise(work)

// MUST TRIGGER (with quick fix): a plain ManagedRuntime that may be undefined
declare const maybeRuntime: MR | undefined
void maybeRuntime?.runPromise(work)

// MUST TRIGGER (with quick fix): a plain ManagedRuntime with a service
declare const serviceRuntime: MR<Service>
void serviceRuntime.runPromise(work)
