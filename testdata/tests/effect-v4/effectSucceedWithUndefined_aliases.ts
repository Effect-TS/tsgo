// @effect-diagnostics *:off
// @effect-diagnostics effectSucceedWithUndefined:suggestion
import { Effect as E } from "effect"
import * as EffectNamespace from "effect/Effect"
import * as Package from "effect"
import { succeed as namedSucceed } from "effect/Effect"

// MUST TRIGGER: renamed module import -> E.undefined
export const renamedEffect = E.succeed(undefined)

// MUST TRIGGER: namespace import of effect/Effect -> E.undefined (the fix uses the file's Effect reference)
export const effectNamespace = EffectNamespace.succeed(undefined)

// MUST TRIGGER: barrel namespace import -> E.undefined
export const packageNamespace = Package.Effect.succeed(undefined)

// MUST TRIGGER: parenthesized receiver -> E.undefined
export const parenthesizedReceiver = (E).succeed(undefined)

// MUST TRIGGER: asserted receiver -> E.undefined
export const assertedReceiver = (E as typeof E).succeed(undefined)

// MUST TRIGGER: parenthesized callee -> E.undefined
export const parenthesizedCallee = (E.succeed)(undefined)

// MUST TRIGGER: named import of succeed -> E.undefined
export const namedImport = namedSucceed(undefined)

// MUST TRIGGER: const alias of a named import -> E.undefined
const renamedSucceedAlias = namedSucceed
export const namedImportAlias = renamedSucceedAlias(undefined)

// MUST TRIGGER: const alias of the module -> E.undefined
const moduleAlias = E
export const constantModuleAlias = moduleAlias.succeed(undefined)

// MUST TRIGGER: the getter receiver is dropped with the callee -> E.undefined
const effectGetter = { get module() { console.log("evaluated"); return E } }
export const getterReceiver = effectGetter.module.succeed(undefined)

// MUST TRIGGER: the called receiver is dropped with the callee -> E.undefined
const getEffect = () => { console.log("evaluated"); return E }
export const calledReceiver = getEffect().succeed(undefined)

// MUST TRIGGER: `void 0` is also Effect<undefined> -> E.undefined
export const namespaceVoid0 = EffectNamespace.succeed(void 0)

// MUST NOT TRIGGER: explicit void type argument belongs to effectSucceedWithVoid, which is off in this file
export const namespaceExplicitVoid = EffectNamespace.succeed<void>(undefined)

// MUST NOT TRIGGER: a parameter named `undefined` shadows the global, this is Effect<string>
export const shadowedUndefined = (undefined: string) => E.succeed(undefined)
