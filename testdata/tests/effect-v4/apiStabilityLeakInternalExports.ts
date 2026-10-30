// @filename: leakyDep.ts
/** @stability experimental */
interface E { value: string }
export interface Leaky { value: E }

// @filename: hiddenDep.ts
/** @stability experimental */
interface E { value: string }
/** @internal */
export interface Hidden { value: E }
export interface Shown { value: E }

// @filename: internalLeakyDep.ts
/** @stability experimental */
interface E { value: string }
/** @internal */
export interface Leaky { value: E }

// @filename: pairDep.ts
/** @stability experimental */
interface E { value: string }
export interface Hidden { value: E }
export interface Shown { value: E }

// @filename: twoLeakyDep.ts
/** @stability experimental */
interface E { value: string }
export interface LeakyOne { value: E }
export interface LeakyTwo { value: E }

// @filename: leakyStarBarrel.ts
export * from "./leakyDep"

// @filename: local.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
/** @stability experimental */
interface E { value: string }
declare const e: E

// An export whose own declaration is tagged `@internal` is not checked, across
// declaration kinds, while its public peers still are.
/** @internal */
export declare const hidden: E
export declare const shown: E
/** @internal */
export const hiddenVar: E = e
/** @internal */
export declare function hiddenFunction(): E
/** @internal */
export interface HiddenInterface { value: E }
/** @internal */
export type HiddenType = { value: E }
/** @internal */
export declare class HiddenClass { value: E }

// Public overloads stay checked when only the implementation is internal.
export function overloaded(x: E): E
export function overloaded(x: string): string
/** @internal */
export function overloaded(x: E | string): E | string {
  return x as E
}
/** @internal */
export declare function gated(x: E): E
/** @internal */
export declare function gated(x: string): string

// Merged declarations are internal only when every declaration is.
/** @internal */
export interface Merged { a: E }
export interface Merged { b: string }
/** @internal */
export interface MergedHidden { a: E }
/** @internal */
export interface MergedHidden { b: string }

// Prose is not a tag.
/** This representation is internalized and @internally tracked. */
export interface Prose { value: E }

// A public signature exposing an internal type still reports.
/** @internal @stability experimental */
interface InternalE { value: string }
/** @internal @stability unstable */
export interface InternalU { value: string }
export interface PublicSurface { value: InternalE }
export interface PublicOther { value: InternalU }

// Internal exports never poison public peers, whichever is visited first.
/** @internal */
export declare const aHidden: E
export declare const zShown: E
export declare const aShown: E
/** @internal */
export declare const zHidden: E

// @filename: defaultFunction.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
/** @stability experimental */
interface E { value: string }
/** @internal */
export default function hiddenDefault(): E {
  return undefined as unknown as E
}

// @filename: defaultAssignment.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
/** @stability experimental */
interface E { value: string }
declare const e: E
/** @internal */
export default e

// @filename: namedTagged.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
/** @internal */
export { Leaky } from "./leakyDep"

// @filename: namedInternalTarget.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
export { Hidden } from "./hiddenDep"
export { Shown } from "./hiddenDep"

// @filename: namedTaggedMany.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
/** @internal */
export { LeakyOne, LeakyTwo } from "./twoLeakyDep"

// @filename: specifierTagged.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
export {
  /** @internal */
  Hidden,
  Shown
} from "./pairDep"

// @filename: starTagged.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
/** @internal */
export * from "./leakyDep"

// @filename: starInternalTarget.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
export * from "./hiddenDep"

// @filename: namespaceTagged.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
/** @internal */
export * as api from "./leakyDep"

// @filename: nsHiddenDep.ts
/** @stability experimental */
interface E { value: string }
/** @stability experimental */
interface F { value: string }
/** @internal */
export interface Hidden { value: E }
/** @internal @stability experimental */
export interface HiddenTagged { value: string }
export interface Shown { value: F }

// @filename: namespaceInternalMembers.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
// Only `F` is reported: internal members are not walked.
export * as api from "./nsHiddenDep"

// @filename: internalLeakyBarrel.ts
export { Leaky } from "./internalLeakyDep"

// @filename: chainInternalTarget.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
export { Leaky } from "./internalLeakyBarrel"

// @filename: taggedLeakyBarrel.ts
/** @internal */
export { Leaky } from "./leakyDep"

// @filename: chainInternalBarrel.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
export { Leaky } from "./taggedLeakyBarrel"

// @filename: cyclicInternal.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
/** @stability experimental */
interface E { value: string }
/** @internal */
export interface Cyclic { value: E }
export * from "./cyclicInternalDep"

// @filename: cyclicInternalDep.ts
export * from "./cyclicInternal"

// @filename: cyclicPublic.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
export * from "./cyclicPublicDep"

// @filename: cyclicPublicDep.ts
export * from "./cyclicPublic"
/** @stability experimental */
interface E { value: string }
export interface CyclicPublic { value: E }

// @filename: orderInternalFirst.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
/** @stability experimental */
interface E { value: string }
/** @internal */
export interface Hidden { value: E }
export interface Shown { value: E }
export * from "./orderInternalFirstDep"

// @filename: orderInternalFirstDep.ts
export * from "./orderInternalFirst"

// @filename: orderPublicFirst.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
/** @stability experimental */
interface E { value: string }
export interface Shown { value: E }
/** @internal */
export interface Hidden { value: E }
export * from "./orderPublicFirstDep"

// @filename: orderPublicFirstDep.ts
export * from "./orderPublicFirst"

// @filename: precedenceNamed.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
// An explicit `@internal` export wins over a redundant star forward.
/** @internal */
export { Leaky } from "./leakyDep"
export * from "./leakyDep"

// @filename: precedenceLocal.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
/** @stability experimental */
interface E { value: string }
/** @internal */
export interface Leaky { value: E }
export * from "./precedenceLocalDep"

// @filename: precedenceLocalDep.ts
export { Leaky } from "./precedenceLocal"

// @filename: precedenceSpecifier.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
export {
  /** @internal */
  Leaky
} from "./leakyDep"
export * from "./leakyDep"

// @filename: precedenceStarChain.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
/** @internal */
export { Leaky } from "./leakyDep"
export * from "./leakyStarBarrel"

// @filename: precedencePublicLocal.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
// A public explicit export stays checked even when a redundant star is internal.
/** @stability experimental */
interface E { value: string }
export interface Leaky { value: E }
/** @internal */
export * from "./precedencePublicLocalDep"

// @filename: precedencePublicLocalDep.ts
export * from "./precedencePublicLocal"

// @filename: precedencePublicForward.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
export { Leaky } from "./leakyDep"
/** @internal */
export * from "./leakyStarBarrel"

// @filename: precedenceUntaggedStarChain.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
/** @stability experimental */
interface E { value: string }
/** @internal */
export interface Leaky { value: E }
export * from "./precedenceUntaggedStarChainOther"

// @filename: precedenceUntaggedStarChainOther.ts
export * from "./precedenceUntaggedStarChainDep"

// @filename: precedenceUntaggedStarChainDep.ts
export { Leaky } from "./precedenceUntaggedStarChain"

// @filename: namespaces.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
// Declared namespaces filter their `@internal` members like namespace
// re-exports, while public members are still walked.
/** @stability experimental */
interface E { value: string }
/** @stability experimental */
interface F { value: string }
export namespace ApiFiltered {
  /** @internal */
  export interface Hidden { value: E }
  export interface Shown { value: string }
}
export namespace ApiPeer {
  /** @internal */
  export interface Hidden { value: E }
  export interface Shown { value: F }
}
export namespace ApiConstant {
  export const safe = "ok"
  /** @internal */
  export interface Hidden { value: E }
}
export namespace ApiMerged {
  /** @internal */
  export interface Hidden { value: E }
}
export namespace ApiMerged {
  export interface Shown { value: F }
}
export namespace Outer.Inner {
  /** @internal */
  export interface Hidden { value: E }
  export interface Shown { value: F }
}
declare namespace Ambient {
  /** @internal */
  export interface Hidden { value: E }
  export interface Shown { value: F }
}
export { Ambient }

// @filename: nestedNamespaceDep.ts
/** @stability experimental */
interface E { value: string }
/** @stability experimental */
interface F { value: string }
export namespace Nested {
  /** @internal */
  export interface Hidden { value: E }
  export interface Shown { value: F }
}

// @filename: namespaceForwarded.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
export * as api from "./nestedNamespaceDep"

// @filename: hiddenDefaultDep.ts
/** @stability experimental */
interface E { value: string }
/** @internal */
export default function hidden(): E { return null as unknown as E }

// @filename: shownDefaultDep.ts
/** @stability experimental */
interface E { value: string }
export default function shown(): E { return null as unknown as E }

// @filename: hiddenFunctionDep.ts
/** @stability experimental */
interface E { value: string }
/** @internal */
export function Hidden(): E { return null as unknown as E }

// @filename: hiddenBarrel.ts
export { Hidden } from "./hiddenDep"

// @filename: hiddenLocalReexportDep.ts
/** @stability experimental */
interface E { value: string }
/** @internal */
interface Hidden { value: E }
export { Hidden }

// @filename: mergedDep.ts
/** @stability experimental */
interface E { value: string }
/** @internal */
export interface Merged { a: E }
export interface Merged { b: string }

// @filename: importNamed.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
// Import-then-export aliases follow the import target's `@internal` tag.
import { Hidden, Shown } from "./hiddenDep"
export { Hidden }
export { Shown }

// @filename: importRenamed.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
import { Hidden as Alias } from "./hiddenDep"
export { Alias }

// @filename: importDefault.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
import Hidden from "./hiddenDefaultDep"
export default Hidden

// @filename: importDefaultPublic.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
import Shown from "./shownDefaultDep"
export default Shown

// @filename: importNamedAsDefault.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
import { Hidden } from "./hiddenFunctionDep"
export default Hidden

// @filename: importBarrel.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
import { Hidden } from "./hiddenBarrel"
export { Hidden }

// @filename: importUntaggedReexport.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
import { Hidden } from "./hiddenLocalReexportDep"
export { Hidden }

// @filename: importMerged.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
import { Merged } from "./mergedDep"
export { Merged }

// @filename: importNamespace.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
import * as ns from "./internalLeakyDep"
export { ns }

// @filename: importTaggedExport.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
import { Shown } from "./hiddenDep"
/** @internal */
export { Shown }

// @filename: importEqualsLocal.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
// `export import X = ...` aliases follow the target named by their reference.
/** @stability experimental */
interface E { value: string }
namespace Impl {
  /** @internal */
  export interface Hidden { value: E }
  export interface Shown { value: E }
}
export import Hidden = Impl.Hidden
export import Shown = Impl.Shown
export { Shown as NormalAlias }
/** @internal */
export import TaggedAlias = Impl.Shown
import Alias = Impl
export import ChainHidden = Alias.Hidden
export import ChainShown = Alias.Shown
export namespace Outer.Inner {
  /** @internal */
  export interface Hidden { value: E }
  export interface Shown { value: E }
}
export import DottedHidden = Outer.Inner.Hidden
export import DottedShown = Outer.Inner.Shown

// @filename: implNamespaceDep.ts
/** @stability experimental */
interface E { value: string }
export namespace Impl {
  /** @internal */
  export interface Hidden { value: E }
  export interface Shown { value: E }
}

// @filename: importEqualsCrossFile.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
import { Impl } from "./implNamespaceDep"
export import Hidden = Impl.Hidden
export import Shown = Impl.Shown

// @filename: importEqualsNamespaceImport.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
import * as Dep from "./hiddenDep"
export import Hidden = Dep.Hidden
export import Shown = Dep.Shown

// @filename: importEqualsRequire.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
import Dep = require("./hiddenDep")
export import Hidden = Dep.Hidden
export import Shown = Dep.Shown

// @filename: internalSafeDep.ts
/** @stability experimental */
interface E { value: string }
/** @internal */
export interface Hidden { value: E }
export interface Safe { value: string }

// @filename: importEqualsWholeModule.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
import Dep = require("./internalSafeDep")
export import Api = Dep

// @filename: importEqualsWholeModulePublic.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
import Dep = require("./hiddenDep")
export import Api = Dep
