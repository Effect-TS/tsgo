// @filename: members.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
/** @stability experimental */
interface E { value: string }

// `@internal` members are excluded from the public surface.
export interface InterfaceProperty {
  /** @internal */ hidden?: E
  visible?: string
}
export interface TaggedInternalProperty {
  /** @internal
   * @stability experimental */ hidden: E
}
export class ClassProperties {
  /** @internal */ hidden!: E
  /** @internal */ static hidden?: E
}
export class MethodAndAccessor {
  /** @internal */ hidden(): E { return null as unknown as E }
  /** @internal */ get value(): E { return null as unknown as E }
}
interface Base {
  /** @internal */ hidden?: E
}
export interface InheritedInternal extends Base {}
export interface MergedInternal {
  /** @internal */ hidden?: E
}
export interface MergedInternal {
  /** @internal */ hidden?: E
}

// Public peers, merged public declarations and represented generic arguments
// stay checked.
export interface PublicPeer {
  /** @internal */ hidden?: E
  visible?: E
}
interface Generic<T> {
  /** @internal */ hidden?: T
}
export declare const genericArgument: Generic<E>
export interface MergedPublic {
  /** @internal */ hidden?: E
}
export interface MergedPublic { hidden?: E }

// @filename: erased.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
/** @stability experimental */
type E = string
export type AnonymousProperty = {
  /** @internal */ hidden?: E
  visible?: string
}
export type AnonymousMethod = {
  /** @internal */ hidden(): E
}

// @filename: internalReferenced.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
// An internal referenced type is still reported through a public member.
/** @internal
 * @stability experimental */
interface E { value: string }
export interface Visible { visible: E }

// @filename: schemaInternal.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
export namespace Annotations {
  export interface Augment { readonly title?: string }
}
export interface Expanded extends Annotations.Augment {}

// @filename: augmentInternal.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
// Internal payloads of an augmented annotation interface do not leak.
import { Annotations } from "./schemaInternal"
/** @stability experimental */
interface Encoding { readonly kind: string }
/** @stability unstable */
interface WithHeadersAnnotation { readonly headers: object }
declare module "./schemaInternal" {
  namespace Annotations {
    interface Augment {
      readonly httpApiStatus?: number | undefined
      /** @internal */
      readonly "~httpApiEncoding"?: Encoding | undefined
      /** @internal */
      readonly "~httpApiWithHeaders"?: WithHeadersAnnotation | undefined
    }
  }
}
export interface Api extends Annotations.Augment {}
export declare const api: Api
export type Literal = {
  /** @internal */ encoding?: Encoding
  readonly httpApiStatus?: number
}

// @filename: schemaPublic.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
export namespace Annotations {
  export interface Augment { readonly title?: string }
}
export interface Expanded extends Annotations.Augment {}

// @filename: augmentPublic.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
// A public payload of the same augmentation reports `Encoding` everywhere.
import { Annotations } from "./schemaPublic"
/** @stability experimental */
interface Encoding { readonly kind: string }
/** @stability unstable */
interface WithHeadersAnnotation { readonly headers: object }
declare module "./schemaPublic" {
  namespace Annotations {
    interface Augment {
      readonly httpApiStatus?: number | undefined
      /** Encoding payload. */
      readonly "~httpApiEncoding"?: Encoding | undefined
      /** @internal */
      readonly "~httpApiWithHeaders"?: WithHeadersAnnotation | undefined
    }
  }
}
export interface Api extends Annotations.Augment {}
export declare const api: Api
export type Literal = {
  /** @internal */ encoding?: Encoding
  readonly httpApiStatus?: number
}
