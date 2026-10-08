// @filename: preferEffectArray.ts
// @effect-diagnostics *:off
// @effect-diagnostics preferEffectArray:warning
import * as A from "effect/Array"
import * as Order from "effect/Order"
import { Array as EffectArray, Effect } from "effect"

const xs = [1, 2, 3]
const strings = ["a", "b"]
xs.map(n => n + 1)
const native_map = xs.map
xs.filter(n => n > 1)
const native_filter = xs.filter
xs.every(n => n > 1)
const native_every = xs.every
xs.some(n => n > 1)
const native_some = xs.some
xs.forEach(n => { void n })
const native_forEach = xs.forEach
xs.flatMap(n => [n, n])
const native_flatMap = xs.flatMap
xs.reduce((acc, n) => acc + n, 0)
const native_reduce = xs.reduce
xs.reduceRight((acc, n) => acc + n, 0)
const native_reduceRight = xs.reduceRight
xs.find(n => n > 1)
const native_find = xs.find
xs.findLast(n => n > 1)
const native_findLast = xs.findLast
xs.findIndex(n => n > 1)
const native_findIndex = xs.findIndex
xs.findLastIndex(n => n > 1)
const native_findLastIndex = xs.findLastIndex
strings.join(",")
const native_join = strings.join
xs.toReversed()
const native_toReversed = xs.toReversed
xs.toSorted((a, b) => a - b)
const native_toSorted = xs.toSorted
Array.isArray(xs)
const native_isArray = Array.isArray

const readonly: ReadonlyArray<number> = xs
const tuple: [number, string] = [1, "a"]
const readonlyTuple: readonly [number, string] = tuple
readonly.map(n => n + 1)
tuple.filter(value => typeof value === "number")
readonlyTuple.map(value => String(value))
function constrained<T extends readonly number[]>(values: T) {
  values["map"](n => n + 1)
  return values.map(n => n + 1)
}
declare const union: number[] | readonly string[]
union.map(value => String(value))
declare const optional: readonly number[] | undefined
optional?.map(n => n + 1)
optional?.["filter"](n => n > 1)
xs.map?.(n => n + 1)
xs["map"](n => n + 1)
const bracket = xs["find"]
readonlyTuple["map"](value => String(value))
;(xs).map(n => n + 1)
Array.prototype.map.call(xs, n => n)
const NativeArray = Array
NativeArray.isArray(xs)
Array["isArray"](xs)
globalThis.Array.isArray(xs)
class Inherited extends Array<number> {}
new Inherited().map(n => n + 1)
const alias = xs.map
alias.call(xs, n => n + 1)
const { map: destructured } = xs
destructured.call(xs, n => n + 1)
export const insideEffect = Effect.gen(function*() {
  return xs.filter(n => n > 1)
})

A.map(xs, n => n + 1)
A.filter(xs, n => n > 1)
A.every(xs, n => n > 1)
A.some(xs, n => n > 1)
A.forEach(xs, n => { void n })
A.flatMap(xs, n => [n, n])
A.reduce(xs, 0, (acc, n) => acc + n)
A.reduceRight(xs, 0, (acc, n) => acc + n)
A.findFirst(xs, n => n > 1)
A.findLast(xs, n => n > 1)
A.findFirstIndex(xs, n => n > 1)
A.findLastIndex(xs, n => n > 1)
A.join(strings, ",")
A.reverse(xs)
A.sort(xs, Order.Number)
A.isArray(xs)
EffectArray.map(xs, n => n + 1)
const EffectAlias = A
EffectAlias.filter(xs, n => n > 1)

const custom = { map: (f: (n: number) => number) => f(1), filter: () => true }
custom.map(n => n + 1)
custom.filter()
new Uint8Array([1, 2]).map(n => n + 1)
new Float64Array([1, 2]).filter(n => n > 1)
class Override extends Array<number> {
  map<U>(f: (n: number, i: number, a: number[]) => U): U[] { return [] }
  callSuper() { return super.map(n => n + 1) }
}
new Override().map(n => n + 1)
declare const mixed: number[] | { map: (f: (n: number) => number) => number[] }
mixed.map(n => n + 1)
declare const intersection: number[] & { map: (f: (n: number) => number) => number[] }
intersection.map(n => n + 1)
function shadowedGlobals() {
  const Array = { isArray: (value: unknown) => true }
  Array.isArray(xs)
}
namespace ShadowedTypes {
  export interface Array<T> { map: (f: (value: T) => T) => T[] }
  export interface ReadonlyArray<T> { filter: (f: (value: T) => boolean) => T[] }
  declare const customArray: Array<number>
  declare const customReadonly: ReadonlyArray<number>
  customArray.map(n => n + 1)
  customReadonly.filter(n => n > 1)
}
xs.map = native_map
xs["filter"] = native_filter
;({ value: xs.map } = { value: native_map })
type MapType = typeof xs.map
type BracketType = typeof xs["filter"]
function typePosition<T extends typeof xs.map>() {}

void xs.at
void xs.concat
void xs.copyWithin
void xs.entries
void xs.fill
void xs.flat
void xs.includes
void xs.indexOf
void xs.keys
void xs.lastIndexOf
void xs.pop
void xs.push
void xs.reverse
void xs.shift
void xs.slice
void xs.sort
void xs.splice
void xs.toLocaleString
void xs.toSpliced
void xs.toString
void xs.unshift
void xs.values
void xs.with
void xs[Symbol.iterator]
void Array.from
void Array.of
void Array.fromAsync
declare const dynamicName: keyof number[]
void xs[dynamicName]
void xs.length
void Array.prototype

// @filename: aliases.ts
// @effect-diagnostics *:off
export const Constructor = Array
// @filename: imported.ts
// @effect-diagnostics *:off
// @effect-diagnostics preferEffectArray:warning
import { Constructor } from "./aliases.js"
Constructor.isArray([])
