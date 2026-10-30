// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning

/** @stability experimental */
interface E { value: string }

// An export's own tag governs its ceiling and is never reported as its own
// dependency, while its children keep being reported.
/** @stability unstable */
export declare function taggedFunction(x: string): E
/** @stability unstable */
export declare function cleanFunction(x: string): string
/** @stability unstable */
export interface TaggedRoot { member: E }
/** @stability unstable */
export type TaggedAliasRoot = { member: E }
/** @stability unstable */
export declare function taggedGeneric<T extends E>(): T
/** @stability experimental */
export interface Shared { value: string }
export interface SharedHolder { shared: Shared }

// An explicit `@stability stable` root still enforces its ceiling.
/** @stability stable */
export declare const stableRoot: E

// A child with an explicit tag contributes its declared level and its
// internals are not reported through the parent; untagged children compose.
/** @stability stable */
export interface StableBase { secret: E }
export interface StableBaseChild extends StableBase {}
/** @stability unstable */
export interface UnstableBase { secret: E }
export interface UnstableBaseChild extends UnstableBase {}
interface PlainBase { secret: E }
export interface PlainChild extends PlainBase {}

/** @stability stable */
export interface TaggedCallable { (): E }
export interface CallableHolder { callable: TaggedCallable }
interface PlainCallable { (): E }
export interface PlainCallableHolder { callable: PlainCallable }

export interface TaggedMemberHolder {
  /** @stability stable */
  nested: { inner: E }
}
export interface MemberHolder {
  nested: { inner: E }
}
export interface TaggedCallSignatureHolder {
  /** @stability stable */
  (): E
}
export interface CallSignatureHolder {
  (): E
}
export interface TaggedIndexHolder {
  /** @stability stable */
  [key: string]: E
}
export interface IndexHolder {
  [key: string]: E
}

/** @stability unstable */
export interface TaggedChild { secret: E }
export interface ChildHolder { child: TaggedChild }

// Generic arguments of a tagged child stay exposed.
/** @stability stable */
export interface StableBox<T> { value: T }
export declare const boxed: StableBox<E>
export interface BoxHolder { box: StableBox<E> }
/** @stability stable */
export interface TaggedChain<T> { value: T }
export interface Mid<U> extends TaggedChain<U> {}
export interface DeepChain extends Mid<E> {}

// A same-named own override is never suppressed by a tagged base.
/** @stability stable */
export interface OverriddenBase { secret: E }
export interface Overriding extends OverriddenBase { secret: E }

// Call, construct and index signatures flattened into a derived type keep
// their tagged base provenance.
/** @stability experimental */
interface FixCallE { value: string }
/** @stability unstable */
export interface FixTaggedCallableBase { (): FixCallE }
export interface FixDerivedCallableGeneric<T> extends FixTaggedCallableBase { own: T }
export declare const fixCallRef: FixDerivedCallableGeneric<string>
interface FixPlainCallableBase { (): FixCallE }
export interface FixPlainDerivedCallableGeneric<T> extends FixPlainCallableBase { own: T }
export declare const fixPlainCallRef: FixPlainDerivedCallableGeneric<string>

/** @stability experimental */
interface FixIdxE { value: string }
/** @stability unstable */
export interface FixTaggedIndexBase { [key: string]: FixIdxE }
export interface FixDerivedIndexGeneric<T> extends FixTaggedIndexBase { own: T }
export declare const fixIndexRef: FixDerivedIndexGeneric<string>
interface FixPlainIndexBase { [key: string]: FixIdxE }
export interface FixPlainDerivedIndexGeneric<T> extends FixPlainIndexBase { own: T }
export declare const fixPlainIndexRef: FixPlainDerivedIndexGeneric<string>

/** @stability experimental */
interface FixComboCallE { value: string }
/** @stability experimental */
interface FixComboCtorE { value: string }
/** @stability experimental */
interface FixComboIdxE { value: string }
/** @stability unstable */
export interface FixComboBase {
  (): FixComboCallE
  new (): FixComboCtorE
  [key: string]: FixComboIdxE
}
export interface FixComboDerived<T> extends FixComboBase { own: T }
export declare const fixComboRef: FixComboDerived<string>

/** @stability experimental */
interface FixCtorE { value: string }
/** @stability experimental */
interface FixStaticE { value: string }
/** @stability experimental */
interface FixMethodE { value: string }
/** @stability unstable */
export declare class FixTaggedBaseClass {
  constructor(x: FixCtorE)
  static s: FixStaticE
  m(): FixMethodE
}
export class FixDerivedClass extends FixTaggedBaseClass {}

/** @stability experimental */
interface FixOwnCtorE { value: string }
/** @stability unstable */
export declare class FixOwnCtorBase { constructor(x: FixOwnCtorE) }
export class FixOwnCtorDerived extends FixOwnCtorBase {
  constructor(y: E) { super(y as unknown as FixOwnCtorE) }
}

/** @stability experimental */
interface FixDeepE { value: string }
/** @stability unstable */
export interface FixTaggedDeepCallable { (): FixDeepE }
export interface FixMidCallable<T> extends FixTaggedDeepCallable { mid: T }
export interface FixDeepCallable<T> extends FixMidCallable<T> {}
export declare const fixDeepCallRef: FixDeepCallable<string>

/** @stability experimental */
interface FixStableE { value: string }
/** @stability stable */
export interface FixStableCallableBase { (): FixStableE }
export interface FixDerivedStableCallable extends FixStableCallableBase {}
export declare const fixStableRef: FixDerivedStableCallable

/** @stability experimental */
interface FixOverrideCallE { value: string }
/** @stability experimental */
interface FixOverrideIdxE { value: string }
/** @stability unstable */
export interface FixOverrideBase { (): FixOverrideCallE; [key: string]: FixOverrideIdxE }
export interface FixOverrideDerived extends FixOverrideBase {
  (): E
  [key: string]: E
}

/** @stability experimental */
interface FixBoxHiddenE { value: string }
/** @stability experimental */
interface FixBoxArgE { value: string }
/** @stability unstable */
export interface FixTaggedCallableBox<T> { (): FixBoxHiddenE; value: T }
export interface FixDerivedCallableBox<T> extends FixTaggedCallableBox<T> {}
export declare const fixCallableBoxRef: FixDerivedCallableBox<FixBoxArgE>

/** @stability experimental */
interface FixBlockedHiddenE { value: string }
/** @stability unstable */
export interface FixBlockedBox<T> { value: T; hidden: FixBlockedHiddenE }
export interface FixBlockedDerived<T, K extends keyof T> extends FixBlockedBox<T[K]> {}
