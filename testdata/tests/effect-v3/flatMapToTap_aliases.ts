import type { Effect } from "effect"
import { as as restore, flatMap as bind, map, tap as observe } from "effect/Effect"
import * as Library from "effect"

declare const source: Effect.Effect<number>
declare const audit: Effect.Effect<void>
const aTap: unknown = observe
const Module = Library.Effect

export const namedTap = source.pipe(bind((n) => restore(audit, n)))
const inspect = observe
export const constTap = source.pipe(bind((n) => map(audit, () => n)))
export const packageNamespace = source.pipe(Library.Effect.andThen((n) => Library.Effect.as(audit, n)))
export const shadowedModule = ((Library: number) => source.pipe(bind((n) => restore(audit, n))))(0)
export const shadowedTap = ((observe: number) => source.pipe(bind((n) => restore(audit, n))))(0)
export const moduleInScope = ((Library: number, observe: number, inspect: number) => source.pipe(bind((n) => restore(audit, n))))(0, 0, 0)
void inspect
void aTap
