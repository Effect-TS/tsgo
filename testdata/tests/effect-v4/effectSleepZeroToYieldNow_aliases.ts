import { Effect as E, Duration as D } from "effect"
import * as EffectNamespace from "effect/Effect"
import * as DurationNamespace from "effect/Duration"
import * as Package from "effect"
import { sleep as namedSleep } from "effect/Effect"

export const renamedEffect = E.sleep(0)
export const renamedDuration = E.sleep(D.zero)
export const renamedConstructor = E.sleep(D.seconds(0))
export const effectNamespace = EffectNamespace.sleep(0)
export const durationNamespace = EffectNamespace.sleep(DurationNamespace.zero)
export const packageNamespace = Package.Effect.sleep(Package.Duration.seconds(0))
export const parenthesizedReceiver = (E).sleep(0)
export const assertedReceiver = (E as typeof E).sleep(0)
export const parenthesizedCallee = (E.sleep)(0)
export const namedImport = namedSleep(0)
const sleepAlias = E.sleep
export const constantAlias = sleepAlias(0)
const renamedSleepAlias = namedSleep
export const namedImportAlias = renamedSleepAlias(0)
const moduleAlias = E
export const constantModuleAlias = moduleAlias.sleep(0)
const effectGetter = { get module() { console.log("evaluated"); return E } }
export const getterReceiver = effectGetter.module.sleep(0)
const getEffect = () => { console.log("evaluated"); return E }
export const calledReceiver = getEffect().sleep(0)
const durationGetter = { get module() { console.log("evaluated"); return D } }
export const getterDurationReceiver = E.sleep(durationGetter.module.zero)
const getDuration = () => { console.log("evaluated"); return D }
export const calledDurationReceiver = E.sleep(getDuration().seconds(0))
