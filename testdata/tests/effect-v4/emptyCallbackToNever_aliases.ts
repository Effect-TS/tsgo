// @effect-diagnostics *:off
// @effect-diagnostics emptyCallbackToNever:warning
import { Effect as E } from "effect"
import * as EffectNamespace from "effect/Effect"
import * as Package from "effect"
import { callback as namedCallback } from "effect/Effect"

// MUST TRIGGER: renamed Effect import (fix: E.never)
export const renamedEffect = E.callback<never>(() => {})

// MUST TRIGGER: namespace import of effect/Effect (fix: EffectNamespace.never)
export const effectNamespace = EffectNamespace.callback<never>(() => {})

// MUST TRIGGER: namespace import of the package (fix: Package.Effect.never)
export const packageNamespace = Package.Effect.callback<never>(() => {})

// MUST TRIGGER: parenthesized receiver
export const parenthesizedReceiver = (E).callback<never>(() => {})

// MUST TRIGGER: asserted receiver
export const assertedReceiver = (E as typeof E).callback<never>(() => {})

// MUST TRIGGER: parenthesized callee
export const parenthesizedCallee = (E.callback<never>)(() => {})

// MUST TRIGGER: named import of callback
export const namedImport = namedCallback<never>(() => {})

// MUST TRIGGER: constant alias of Effect.callback
const callbackAlias = E.callback
export const constantAlias = callbackAlias<never>(() => {})

// MUST TRIGGER: constant alias of the Effect module
const moduleAlias = E
export const constantModuleAlias = moduleAlias.callback<never>(() => {})

// MUST TRIGGER: receiver evaluated through a getter (diagnostic only, a fix would drop the getter's side effect)
const effectGetter = { get module() { console.log("evaluated"); return E } }
export const getterReceiver = effectGetter.module.callback<never>(() => {})

// MUST TRIGGER: receiver evaluated through a call (diagnostic only, a fix would drop the call's side effect)
const getEffect = () => { console.log("evaluated"); return E }
export const calledReceiver = getEffect().callback<never>(() => {})
