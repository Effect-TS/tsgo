// @filename: /node_modules/effect/dist/NarrowCtor.d.ts
import type { Effect } from "./Effect.ts";
/**
 * Synthetic effect module for the rule boundary: `makeUnsafe` returns a type
 * narrower than `make`'s success type, so rewriting would change the
 * expression's inferred type.
 */
export declare const make: (value: string) => Effect<string>;
export declare const makeUnsafe: (value: string) => "narrow";
// @filename: /node_modules/effect/dist/GenCtor.d.ts
import type { Effect } from "./Effect.ts";
/**
 * Synthetic effect module for the rule boundary: both returns share the
 * `ReadonlyArray` origin, but `makeUnsafe` instantiates it with the inferred
 * literal `A` while `make`'s success type fixes it to `string`, so rewriting
 * narrows the expression to `ReadonlyArray<"x">`.
 */
export declare const make: <A extends string>(a: A) => Effect<ReadonlyArray<string>>;
export declare const makeUnsafe: <A extends string>(a: A) => ReadonlyArray<A>;
// @filename: /node_modules/effect/dist/ShapeCtor.d.ts
import type { Effect } from "./Effect.ts";
/**
 * Synthetic effect module for the rule boundary: the sibling never mentions its
 * type parameter in its parameters, so a rewritten call site would infer
 * `unknown` for it instead of the constructor's inferred argument type.
 */
export declare const make: <A>(value: A) => Effect<ReadonlyArray<A>>;
export declare const makeUnsafe: <A>(ignored?: unknown) => ReadonlyArray<A>;
// @filename: preferUnsafeConstructor.ts
// @effect-diagnostics *:off
// @effect-diagnostics preferUnsafeConstructor:warning
import { Chunk, Deferred, Effect, Latch, Queue, Ref, Scope, TxChunk } from "effect"
import { runSync } from "effect/Effect"
import { make as makeScope, makeUnsafe as makeScopeUnsafe } from "effect/Scope"
import * as ScopeNs from "effect/Scope"
import * as NarrowCtor from "effect/NarrowCtor"
import * as GenCtor from "effect/GenCtor"
import * as ShapeCtor from "effect/ShapeCtor"

// --- should report: direct constructor call with a matching *Unsafe sibling ---

export const scope = Effect.runSync(Scope.make())

export const scopeParallel = Effect.runSync(Scope.make("parallel"))

export class ResourceHolder {
  readonly scope: Scope.Closeable = Effect.runSync(Scope.make("parallel"))
}

export const ref = Effect.runSync(Ref.make(1))

export const deferred = Effect.runSync(Deferred.make<string>())

export const latch = Effect.runSync(Latch.make(true))

export const namespaceImport = Effect.runSync(ScopeNs.make())

export const namedImport = Effect.runSync(makeScope())

export const aliasedRunSync = runSync(Scope.make())

export const parenthesized = Effect.runSync((Scope.make()))

// --- must not report ---

// Not a direct constructor call: variable reference.
const scopeEffect = Scope.make()
export const fromVariable = Effect.runSync(scopeEffect)

// Not a direct constructor call: composed effect.
export const piped = Effect.runSync(Scope.make().pipe(Effect.map((s) => s)))

// Effect.gen has no genUnsafe sibling.
export const fromGen = Effect.runSync(Effect.gen(function*() {
  return yield* Ref.make(0)
}))

// Effect.succeed has no succeedUnsafe sibling.
export const noSibling = Effect.runSync(Effect.succeed(1))

// The *Unsafe variant itself must stay untouched.
export const alreadyUnsafe = Scope.makeUnsafe()
export const alreadyUnsafeNamed = makeScopeUnsafe()

// Inside an Effect generator context, runEffectInsideEffect owns the report.
export const insideGen = Effect.gen(function*() {
  const scope = Effect.runSync(Scope.make())
  return yield* Effect.succeed(scope)
})

// makeUnsafe returns the narrower literal "narrow": assignable to the success
// type `string` but not mutually, so the rewrite would change the inferred type.
export const narrowerReturn = Effect.runSync(NarrowCtor.make("value"))

// Same ReadonlyArray origin, but makeUnsafe("x") would narrow the result from
// ReadonlyArray<string> to ReadonlyArray<"x">.
export const genericContainer = Effect.runSync(GenCtor.make("x"))

// makeUnsafe's type parameter is absent from its parameters, so the rewritten
// call site would infer ReadonlyArray<unknown> instead of ReadonlyArray<number>.
export const uninferrableTypeParameter = Effect.runSync(ShapeCtor.make(1))

// TxChunk.makeUnsafe takes a TxRef, not the Chunk that TxChunk.make accepts.
declare const chunk: Chunk.Chunk<number>
export const differentParams = Effect.runSync(TxChunk.make(chunk))

// Queue.takeUnsafe returns `Exit<A, E> | undefined`, not the success type.
declare const queue: Queue.Queue<number, never>
export const differentReturn = Effect.runSync(Queue.take(queue))

// A userland lookalike is not exported from the effect package.
const MyScope = {
  make: (): Effect.Effect<number> => Effect.succeed(1),
  makeUnsafe: (): number => 1,
}
export const userlandLookalike = Effect.runSync(MyScope.make())
