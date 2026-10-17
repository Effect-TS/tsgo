// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning

import * as Data from "effect/Data"

declare const ignored: unique symbol

/** @stability unstable */
export interface Reportable {
  /** @stability unstable */
  readonly [ignored]?: boolean
  /** @stability unstable */
  readonly attributes?: Readonly<Record<string, unknown>>
  /** @internal */
  readonly encoding?: ExperimentalEncoding
}

/** @stability experimental */
interface ExperimentalEncoding {
  readonly kind: string
}

// A direct property or parameter still exposes the unstable interface.
export interface TaggedErrorProperty {
  readonly _tag: string
  readonly a: Reportable
}
export declare function direct(reportable: Reportable): void
export type DirectIntersection = Reportable & { readonly _tag: string }

// Inheritance contributes only optional tagged public members. The required
// _tag belongs to the derived interface and is checked independently.
export interface TaggedErrorInheritance extends Reportable {
  readonly _tag: string
}
export interface TransitiveInheritance extends TaggedErrorInheritance {}
export declare const inherited: TaggedErrorInheritance

// A required member or an untagged optional member prevents the base exception.
/** @stability unstable */
interface RequiredReportable {
  /** @stability unstable */
  readonly required: boolean
}
export interface RequiredInheritance extends RequiredReportable {
  readonly _tag: string
}

/** @stability unstable */
interface UntaggedReportable {
  readonly optional?: boolean
}
export interface UntaggedInheritance extends UntaggedReportable {}

// A derived type's own less-stable members still report.
export interface OwnLeak extends Reportable {
  readonly encoding: ExperimentalEncoding
}

// An Error augmentation and a superclass factory must preserve the same
// inheritance exception, including members flattened from intersection types.
declare global {
  interface Error extends Reportable {}
}
export class TaggedErrorFactory extends Data.TaggedError("TaggedErrorFactory")<{
  readonly message: string
}> {}

declare const makeError: new () => Reportable & { readonly _tag: string }
export class IntersectionSuperclass extends makeError {}

type ConstructorBase = Reportable & { readonly _tag: string }
declare const makeAliasedError: new () => ConstructorBase
export class AliasedIntersectionSuperclass extends makeAliasedError {}

// Internal annotations are excluded regardless of whether they are optional.
export interface InternalPayload {
  /** @internal */
  readonly encoding: ExperimentalEncoding
  readonly status?: number
}
