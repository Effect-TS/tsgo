// @filename: forwardDep.ts
/** @stability experimental */
interface E { value: string }
export interface Leaky { value: E }

// @filename: exportedEDep.ts
/** @stability experimental */
export interface E { value: string }
export interface Leaky { value: E }

// @filename: barrelTsDep.ts
/** @stability experimental */
interface E { value: string }
export declare function f(x: E): void

// @filename: barrelTs.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
export { f } from "./barrelTsDep.js"

// @filename: barrelDtsDep.d.ts
/** @stability experimental */
interface E { value: string }
export declare function f(x: E): void

// @filename: barrelDts.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
export { f } from "./barrelDtsDep.js"

// @filename: starDtsDep.d.ts
/** @stability experimental */
interface E { value: string }
export interface X { value: E }

// @filename: starDts.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
export * from "./starDtsDep.js"

// @filename: nestedNamespaceBarrel.ts
export * as nested from "./forwardDep"

// @filename: nestedNamespace.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
export * as api from "./nestedNamespaceBarrel"

// @filename: namespaceOwnMember.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
// A namespace member's own stability is reported.
export * as api from "./experimentalOnlyDep"

// @filename: experimentalOnlyDep.ts
/** @stability experimental */
export interface E { value: string }

// @filename: importedCallableDep.ts
/** @stability experimental */
interface E { value: string }
export interface Callable { (x: E): E }

// @filename: importedCallable.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
import type { Callable } from "./importedCallableDep"
export interface O { f: Callable }

// @filename: importedAliasDep.ts
/** @stability experimental */
interface E { value: string }
interface Box<T> { value: T }
export type Holder = Box<E>

// @filename: importedAlias.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
import type { Holder } from "./importedAliasDep"
export declare const x: Holder

// @filename: namespaceCycle.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
// Namespace and alias cycles terminate.
export * as api from "./namespaceCycleBarrel"

// @filename: namespaceCycleBarrel.ts
export * as back from "./namespaceCycle"
/** @stability experimental */
interface E { value: string }
export interface Leaky { value: E }

// @filename: aliasCycle.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
export { x } from "./aliasCycleDep"

// @filename: aliasCycleDep.ts
export { x } from "./aliasCycle"

// @filename: unstableDep.ts
/** @stability unstable */
export interface U { value: string }
export interface Leaky { value: U }

// @filename: namespaceUnstable.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
/** @stability unstable */
export * as api from "./unstableDep"

// @filename: starExperimental.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
/** @stability experimental */
export * from "./forwardDep"

// @filename: starUnstable.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
/** @stability unstable */
export * from "./exportedEDep"

// @filename: unstableLeakyDep.ts
/** @stability experimental */
interface E { value: string }
/** @stability unstable */
export interface Leaky { value: E }

// @filename: stableOverUnstable.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
/** @stability stable */
export { Leaky } from "./unstableLeakyDep"

// @filename: matrixDep.d.ts
/** @stability experimental */
interface E { v: string }
/** @stability experimental */
export interface X { v: E }

// @filename: namedStable.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
// An explicit forwarding tag is a ceiling, even when it is `stable`; without a
// tag, named and star re-exports inherit the forwarded symbol's own tag.
/** @stability stable */
export { X } from "./matrixDep.js"

// @filename: namedUnstable.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
/** @stability unstable */
export { X } from "./matrixDep.js"

// @filename: namedExperimental.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
/** @stability experimental */
export { X } from "./matrixDep.js"

// @filename: namedAbsent.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
export { X } from "./matrixDep.js"

// @filename: starStable.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
/** @stability stable */
export * from "./matrixDep.js"

// @filename: starUnstableCeiling.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
/** @stability unstable */
export * from "./matrixDep.js"

// @filename: starExperimentalCeiling.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
/** @stability experimental */
export * from "./matrixDep.js"

// @filename: starAbsent.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
export * from "./matrixDep.js"

// @filename: namespaceStable.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
// A namespace alias without a tag inherits the module's stable default.
/** @stability stable */
export * as ns from "./matrixDep.js"

// @filename: namespaceUnstableCeiling.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
/** @stability unstable */
export * as ns from "./matrixDep.js"

// @filename: namespaceExperimentalCeiling.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
/** @stability experimental */
export * as ns from "./matrixDep.js"

// @filename: namespaceAbsent.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
export * as ns from "./matrixDep.js"

// @filename: inferredFunctionDep.ts
/** @stability experimental */
interface E { v: string }
declare const e: E
export function f() { return e }

// @filename: inferredGenericClosureDep.ts
/** @stability experimental */
interface E { v: string }
declare const e: E
function make<T>(t: T) { return () => ({ v: t }) }
export const f = make(e)

// @filename: inferredDirectClosureDep.ts
/** @stability experimental */
interface E { v: string }
declare const e: E
function make<T>(t: T) { return () => t }
export const f = make(e)

// @filename: inferredFunctionNamed.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
// Inferred re-exports are read through the checker's native inference.
export { f } from "./inferredFunctionDep.js"

// @filename: inferredFunctionStar.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
export * from "./inferredFunctionDep.js"

// @filename: inferredFunctionNamespace.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
export * as ns from "./inferredFunctionDep.js"

// @filename: inferredGenericClosureNamed.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
export { f } from "./inferredGenericClosureDep.js"

// @filename: inferredGenericClosureStar.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
export * from "./inferredGenericClosureDep.js"

// @filename: inferredGenericClosureNamespace.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
export * as ns from "./inferredGenericClosureDep.js"

// @filename: inferredDirectClosureNamed.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
export { f } from "./inferredDirectClosureDep.js"

// @filename: inferredDirectClosureStar.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
export * from "./inferredDirectClosureDep.js"

// @filename: inferredDirectClosureNamespace.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
export * as ns from "./inferredDirectClosureDep.js"

// @filename: objectAccessorDep.ts
/** @stability experimental */
interface E { v: string }
declare const e: E
export const f = { get v() { return e } }

// @filename: objectAccessorNamed.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
// Inferred accessors are reported through every kind of barrel.
export { f } from "./objectAccessorDep.js"

// @filename: objectAccessorStar.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
export * from "./objectAccessorDep.js"

// @filename: objectAccessorNamespace.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
export * as ns from "./objectAccessorDep.js"

// @filename: classAccessorDep.ts
/** @stability experimental */
interface E { v: string }
declare const e: E
export class C { get value() { return e } }

// @filename: classAccessor.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
export { C } from "./classAccessorDep.js"

// @filename: genericClosureAccessorDep.ts
/** @stability experimental */
interface E { v: string }
declare const e: E
function make<T>(t: T) { return { get v() { return t } } }
export const f = make(e)

// @filename: genericClosureAccessor.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
export { f } from "./genericClosureAccessorDep.js"

// @filename: setterDep.ts
/** @stability experimental */
interface E { v: string }
export const f = { set v(x: E) {} }

// @filename: setter.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
export { f } from "./setterDep.js"

// @filename: accessorNameErrorDep.ts
export const f = { get v() { return nope } }

// @filename: accessorNameError.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
// Accessor bodies with errors keep only the compiler's own diagnostics.
export { f } from "./accessorNameErrorDep.js"

// @filename: annotatedAccessorErrorDep.ts
/** @stability experimental */
interface E { v: string }
export const f = { get v(): E { return nope } }

// @filename: annotatedAccessorError.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
export { f } from "./annotatedAccessorErrorDep.js"

// @filename: setterBodyErrorDep.ts
export const f = { set v(x: number) { x.missing } }

// @filename: setterBodyError.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
export { f } from "./setterBodyErrorDep.js"
