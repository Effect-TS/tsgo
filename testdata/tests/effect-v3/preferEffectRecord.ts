// @filename: preferEffectRecord.ts
// @effect-diagnostics *:off
// @effect-diagnostics preferEffectRecord:warning
import * as R from "effect/Record"
import { Effect, Record as EffectRecord } from "effect"
import { NativeObject } from "./aliases"

const data = { a: 1, b: "two" }
const pairs: Array<[string | symbol, number]> = [["a", 1], [Symbol.for("b"), 2]]
Object.keys(data)
const nativeKeys = Object.keys
Object.values(data)
const nativeValues = Object.values
Object.entries(data)
const nativeEntries = Object.entries
Object.fromEntries(pairs)
const nativeFromEntries = Object.fromEntries
Object.hasOwn(data, "a")
const nativeHasOwn = Object.hasOwn
data.hasOwnProperty("a")
const nativeHasOwnProperty = data.hasOwnProperty
Object.prototype.hasOwnProperty.call(data, "a")
Object.hasOwnProperty.call(data, "a")
Object["keys"](data)
Object[`values`](data)
Object.k\u0065ys(data)
Object["entr\u0069es"](data)
Object.entries?.(data)
;(Object).hasOwn(data, "a")
declare const optional: object | undefined
optional?.["hasOwnProperty"]("a")
declare const optionalConstructor: ObjectConstructor | undefined
optionalConstructor?.["keys"](data)
const Native = Object
Native.entries(data)
NativeObject.keys(data)
globalThis.Object.hasOwn(data, "a")
function constrained<T extends ObjectConstructor>(native: T) { return native["keys"](data) }
function constrainedInstance<T extends object>(value: T) { return value.hasOwnProperty("a") }
class Inherited {}
new Inherited().hasOwnProperty("a")
declare const picked: Pick<ObjectConstructor, "entries">
picked.entries(data)
type Remapped = { [K in keyof ObjectConstructor as K extends "values" ? "keys" : never]: ObjectConstructor[K] }
declare const remapped: Remapped
remapped.keys(data)
Object.keys([1, 2])
Object.values("ab")
Object.fromEntries([[1, "a"]])
Object.hasOwn(data, 1)
;[1].hasOwnProperty("0")
"ab".hasOwnProperty("0")
export const program = Effect.gen(function*() { return Object.entries(data) })

const typedKeys: Array<"a" | "b"> = R.keys(data)
const typedEntries: Array<["a" | "b", string | number]> = R.toEntries(data)
R.values(data)
R.fromEntries(pairs)
R.has(data, "a")
EffectRecord.keys(data)
EffectRecord.values(data)
EffectRecord.toEntries(data)
EffectRecord.fromEntries(pairs)
EffectRecord.has(data, "b")
// @ts-expect-error Record keys rejects arrays.
R.keys([1])
// @ts-expect-error Record keys rejects primitive strings.
R.keys("ab")
// @ts-expect-error Record values rejects arrays.
R.values([1])
// @ts-expect-error Record toEntries rejects arrays.
R.toEntries([1])
// @ts-expect-error Record fromEntries requires string or symbol tuple keys.
R.fromEntries([[1, "a"]])
declare const arbitraryKey: string
// @ts-expect-error Record has requires a key of this record's type.
R.has(data, arbitraryKey)
// @ts-expect-error Record has rejects numeric keys.
R.has(data, 1)
// @ts-expect-error Record has rejects arrays.
R.has([1], "0")
nativeKeys(data)
const { keys: destructured } = Object
destructured(data)
type Keys = typeof Object.keys
declare const dynamic: "keys"
Object[dynamic](data)
declare const custom: { keys: typeof Object.keys; hasOwnProperty(key: PropertyKey): boolean }
custom.keys(data)
custom.hasOwnProperty("a")
function shadowed(Object: { keys(value: object): string[] }) { return Object.keys(data) }
class Override { hasOwnProperty(key: PropertyKey): boolean { return false } }
new Override().hasOwnProperty("a")
class Child extends Inherited { test() { return super.hasOwnProperty("a") } }
declare const mixed: ObjectConstructor | { keys: typeof Object.keys }
mixed.keys(data)
declare const mixedInstance: object | { hasOwnProperty(key: PropertyKey): boolean }
mixedInstance.hasOwnProperty("a")
declare const intersection: ObjectConstructor & { keys(value: number): string[] }
intersection.keys(data)
declare const conflicting: Remapped | Pick<ObjectConstructor, "keys">
conflicting.keys(data)
declare let writable: Partial<Pick<ObjectConstructor, "keys">>
writable.keys = () => []
delete (writable.keys)
;({ keys: writable.keys } = Object)
Object.prototype.hasOwnProperty = key => false
void [[1].keys(), [1].values(), [1].entries(), new Map().keys(), new Map().values(), new Map().entries()]
void [Object.assign, Object.create, Object.defineProperties, Object.defineProperty, Object.freeze, Object.getOwnPropertyDescriptor, Object.getOwnPropertyDescriptors, Object.getOwnPropertyNames, Object.getOwnPropertySymbols, Object.getPrototypeOf, Object.groupBy, Object.is, Object.isExtensible, Object.isFrozen, Object.isSealed, Object.preventExtensions, Object.seal, Object.setPrototypeOf, Object.prototype]
void [data.isPrototypeOf, data.propertyIsEnumerable, data.toLocaleString, data.toString, data.valueOf]

// @filename: aliases.ts
// @effect-diagnostics *:off
export const NativeObject = Object
