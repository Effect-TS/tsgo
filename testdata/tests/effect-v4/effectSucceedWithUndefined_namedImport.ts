// @effect-diagnostics *:off
// @effect-diagnostics effectSucceedWithUndefined:suggestion
import type { Effect } from "effect"
import { succeed, succeed as renamedSucceed } from "effect/Effect"

// MUST TRIGGER, diagnostic only: NO replacement quickfix, no value binding reaches Effect.undefined
// (the file imports only `succeed`, and the `Effect` import is type-only)
export const namedImport = succeed(undefined)

// MUST TRIGGER, diagnostic only: NO replacement quickfix, the renamed import does not reach Effect.undefined either
export const renamedImport = renamedSucceed(undefined)

// MUST TRIGGER, diagnostic only: NO replacement quickfix, a const alias of the named import
const succeedAlias = succeed
export const namedImportAlias = succeedAlias(undefined)

// MUST NOT TRIGGER: the type-only import is still usable in types
export const annotated: Effect.Effect<number> = succeed(1)
