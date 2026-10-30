// @filename: computed.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
// Computed member names are not evaluated; their annotations are reported.
/** @stability experimental */
type PropertyAlias = string
/** @stability experimental */
type ParameterAlias = string
/** @stability experimental */
type MethodAlias = string
export declare const x: {
  [Symbol.iterator]?: PropertyAlias
  [Symbol.toStringTag](value: ParameterAlias): MethodAlias
  ["computed"]: PropertyAlias
}
export type Y = { readonly [Symbol.iterator]?: never }
export interface Z { readonly [Symbol.iterator]?: never }

// @filename: defaultArrow.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
/** @stability experimental */
interface E { value: string }
export default (x: E): E => x

// @filename: inferred.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
/** @stability experimental */
interface E { value: string }
declare const e: E
export function nestedResult() { return { nested: { value: e } } }
export const nestedArrowResult = () => ({ nested: { value: e } })
const inferredObject = { value: e }
export type InferredObjectType = typeof inferredObject
export function parameterObject(x = { value: e }): void {}
const inferredArrow = () => e
export declare const typeofInferredArrow: typeof inferredArrow
const inferredNestedClosure = () => () => e
export declare const typeofNestedClosure: typeof inferredNestedClosure

// Closures returned from generic functions expose their concrete binder.
declare function makeThunk<T>(): () => T
export const bareBinder = makeThunk<E>()
function makeObjectClosure<T>(t: T) { return () => ({ v: t }) }
export const objectClosure = makeObjectClosure(e)
function makeDirectClosure<T>(t: T) { return () => t }
export const directClosure = makeDirectClosure(e)
function makeNestedClosure<T>(t: T) { return () => () => t }
export const nestedClosure = makeNestedClosure(e)
function makePropertyClosure<T>(t: T) { return { f: () => t } }
export const propertyClosure = makePropertyClosure(e)

// @filename: bareBinderCleanFirst.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
declare function make<T>(): () => T
export const x = make<E>()
/** @stability experimental */
interface E { v: string }

// @filename: signatures.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
/** @stability experimental */
interface E { value: string }
/** @stability experimental */
type Text = string

// Hidden overload implementations do not leak; public overloads do.
export function hiddenImplementation(x: string): string
export function hiddenImplementation(x: string | E): string { return "" }
export function publicOverload(x: E): E
export function publicOverload(x: string): string
export function publicOverload(x: E | string): E | string { return x as E }

declare function identity<T>(x: T): T
export const instantiationExpression = identity<E>
interface Callable<T> { (x: E): T }
export interface CallableMember { f: Callable<string> }
export declare const callableUnion: Callable<string> | undefined
export class ErasedAccessor {
  get value(): Text { return "x" }
}
interface Box<T> { value: T }
export declare function erasedUnion(x: Text | undefined): void
export declare const erasedArgument: Box<Text>
type Alias<T> = Box<T>
export declare const aliasOfInterface: Alias<E>
type Wrapper<T> = { value: T }
export declare const aliasOfLiteral: Wrapper<E>
declare function fn(x: E): void
export declare const typeofCallable: typeof fn

// Declared member and signature tags contribute with their own provenance.
export interface TaggedProperty {
  /** @stability experimental */
  value: string
}
export interface TaggedCallSignature {
  /** @stability unstable */
  (x: string): string
}
export interface TaggedIndexSignature {
  /** @stability experimental */
  [key: string]: number
}
export declare const taggedAnonymousProperty: {
  /** @stability experimental */
  value: string
}
export declare const taggedAnonymousMethod: {
  /** @stability experimental */
  f(x: string): void
}
export declare const taggedAnonymousCall: {
  /** @stability experimental */
  (x: string): void
}
export type TaggedBranchMember<T> = T extends number ? {
  /** @stability experimental */
  value: string
} : boolean

// Declared conditional branches are represented; defaulted applications
// select the compiler's branch.
export type GenericBranch<T> = T extends string ? E : number
export type GenericArrayBranch<T> = T[] extends string[] ? E : number
export type DeferredErasedBranch<T> = T extends number ? Text : boolean
export type FirstInfer<T> = T extends [infer U, ...unknown[]] ? E : never
export declare const materializedInfer: string[] extends Array<infer U> ? E : never
interface DefaultHolder<T = string> { value: T extends string ? number : E }
export declare const defaultInterface: DefaultHolder
type DefaultAlias<T = string> = T extends string ? number : E
export declare const defaultAlias: DefaultAlias
type Finite<T> = T extends string ? Finite<number> : E
export declare const finiteRecursive: Finite<string>
type Step<T extends unknown[]> = T extends [unknown, ...infer Tail] ? Step<Tail> : E
export declare const finiteTuple: Step<[0, 0]>
interface Indexed { value: E }
type Read<T> = T["value" & keyof T]
export declare const indexedErased: Read<Indexed>

// Erased and flattened aliases keep their provenance in unions and
// intersections.
/** @stability experimental */
type Erased = string
export declare const collapsedUnion: never | number | Erased
export declare const reorderedUnion: number | Erased | never
export declare const collapsedIntersection: Erased & string
/** @stability experimental */
type FlatUnion = string | number
export declare const flattenedUnion: FlatUnion | boolean
/** @stability experimental */
type FlatIntersection = { a: string } & { b: number }
export declare const flattenedIntersection: FlatIntersection & { c: boolean }

// Nested class statics stay shallow; benign defaults keep their argument.
class WithStatic { static value: E }
export interface StaticHolder { ctor: typeof WithStatic }
interface Defaulted<A, B = A> { v: B }
export declare const benignDefault: Defaulted<E>

// @filename: cycleAFirst.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
// Anonymous compound cycles complete their dependencies for every root.
/** @stability experimental */
interface E { value: string }
type B = { c: C; payload: E }
type C = { b: B }
export type A = { b: B }
export type Z = { c: C }
export type SelfA = { b: SelfB; payload: E }
export type SelfB = { a: SelfA }

// @filename: cycleZFirst.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
/** @stability experimental */
interface E { value: string }
type B = { c: C; payload: E }
type C = { b: B }
export type Z = { c: C }
export type A = { b: B }
export type SelfB = { a: SelfA }
export type SelfA = { b: SelfB; payload: E }

// @filename: conditionalReturns.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
/** @stability experimental */
interface E { value: string }
declare function makeObject<T>(): { value: T extends string ? number : E }
export const anonymousConditional = makeObject<number>()
declare function makeCallable<T>(): () => T extends string ? number : E
export const callableConditional = makeCallable<number>()
declare function makeNested<T>(): { f: () => { value: T extends string ? number : E } }
export const nestedConditional = makeNested<number>()

// @filename: conditionalMembers.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
// Instantiated conditional members report the compiler's selected branch.
/** @stability experimental */
interface E { value: string }
interface Contravariant<in T> { f: (x: T) => void }
interface ContravariantHolder<T> { value: T extends Contravariant<string> ? number : E }
export declare const contravariant: ContravariantHolder<Contravariant<"x">>
interface Invariant<in out T> { f: (x: T) => T }
interface InvariantHolder<T> { value: T extends Invariant<string> ? number : E }
export declare const invariant: InvariantHolder<Invariant<"x">>
interface ArrayHolder<T> { value: T extends Contravariant<string>[] ? number : E }
export declare const arrayWrapped: ArrayHolder<Contravariant<"x">[]>
interface StructuralHolder<T> { value: T extends { value: string } ? number : E }
export declare const structural: StructuralHolder<{ value: string }>

// @filename: indexedRepresented.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
/** @stability experimental */
interface E { value: string }
interface I { value: E }
type Read<T> = T["value" & keyof T]
declare function make<T>(): () => Read<T>
export const x = make<I>()
x()

// @filename: indexedLazy.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
/** @stability experimental */
interface E { value: string }
interface I { value: E }
type Read<T> = T["value" & keyof T]
declare function make<T>(): () => Read<T>
export const x = make<I>()

// @filename: shallowNamed.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
// A nested named reference is a shallow dependency, even once materialized.
/** @stability experimental */
interface E { value: string }
interface Hidden<T> { value: E }
declare const h: Hidden<string>
h.value
export interface X { hidden: Hidden<string> }

// @filename: shallowTagged.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
/** @stability experimental */
interface E { value: string }
interface Hidden<T> { /** @stability experimental */ value: string }
declare const h: Hidden<string>
h.value
export interface X { hidden: Hidden<string> }

// @filename: relatedOverload.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
// The related location points at the tagged overload signature.
interface Callable {
  /** @stability experimental */
  (value: string): string;
  (value: number): number;
}
export interface Public { call: Callable }
