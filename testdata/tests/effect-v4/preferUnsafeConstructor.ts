// @filename: /node_modules/effect/dist/NarrowCtor.d.ts
import type { Effect } from "./Effect.ts";
/**
 * Synthetic effect module for the rule boundary: `makeUnsafe` returns a type
 * narrower than `make`'s success type, so rewriting would change the
 * expression's inferred type.
 */
export declare const make: (value: string) => Effect<string>;
export declare const makeUnsafe: (value: string) => "narrow";
// @filename: preferUnsafeConstructor.ts
// @effect-diagnostics *:off
// @effect-diagnostics preferUnsafeConstructor:warning
// @effect-diagnostics runEffectInsideEffect:warning
import { Chunk, Deferred, Effect, Latch, Queue, Ref, Scope, TxChunk } from "effect"
import { runSync } from "effect/Effect"
import { make as makeScope, makeUnsafe as makeScopeUnsafe } from "effect/Scope"
import * as ScopeNs from "effect/Scope"
import * as NarrowCtor from "effect/NarrowCtor"

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

// Inside an Effect generator context, both diagnostics report: this rule flags
// the constructor rewrite and runEffectInsideEffect flags the nested run call.
export const insideGen = Effect.gen(function*() {
  const scope = Effect.runSync(Scope.make())
  return yield* Effect.succeed(scope)
})

// makeUnsafe returns the narrower literal "narrow": assignable to the success
// type `string` but not mutually, so the rewrite would change the inferred type.
export const narrowerReturn = Effect.runSync(NarrowCtor.make("value"))

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
