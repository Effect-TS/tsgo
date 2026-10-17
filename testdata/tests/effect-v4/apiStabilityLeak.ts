// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning

/** @stability unstable */
export interface UnstableType {
  value: string
}

/** @stability experimental */
export interface ExperimentalType {
  value: string
}

/** @stability unstable */
export type UnstableAlias = { value: string }

/** @stability experimental */
export type ExperimentalAlias = { value: string }

// A stable export must not expose unstable or experimental types.
export interface StableExposesUnstable {
  member: UnstableType
}

export interface StableExposesExperimental {
  member: ExperimentalType
}

export function stableFunction(input: ExperimentalType): UnstableType {
  return input as unknown as UnstableType
}

// Public methods and constructors participate in the surface.
export interface StableMethods {
  method(input: ExperimentalType): UnstableType
  new (value: ExperimentalType): StableMethods
}

// Generic target/alias symbols and their type arguments participate too.
export interface Container<T> {
  value: T
}

export type StableWithUnstableArgument = Container<UnstableType>
export type StableWithExperimentalArgument = Container<ExperimentalType>

// Union and intersection constituents are directly represented.
export type StableUnion = UnstableType | string
export type StableIntersection = { tag: string } & ExperimentalType

// Only public class members participate; private and protected members do not.
export class StableClass {
  private secret: ExperimentalType | undefined
  protected guarded: UnstableType | undefined
  public exposed: UnstableType | undefined
}

// An unstable export must not expose experimental types.
/** @stability unstable */
export interface UnstableExposesExperimental {
  member: ExperimentalType
}

// An unstable export may still expose unstable types.
/** @stability unstable */
export interface UnstableExposesUnstable {
  member: UnstableType
}

/** @stability unstable */
export type UnstableWithExperimentalArgument = Container<ExperimentalType>

// An experimental export imposes no restriction.
/** @stability experimental */
export interface ExperimentalExposesEverything {
  unstable: UnstableType
  experimental: ExperimentalType
}

// Inline anonymous object surfaces are directly represented.
export declare function inlineParameter(x: { value: ExperimentalType }): void
export declare function inlineResult(): { value: UnstableType }
export interface InlineNested {
  nested: { value: ExperimentalType }
}
export type InlineCombined = { value: UnstableType } & { other: string }

// An erased alias keeps its tagged symbol even when the checker exposes string.
/** @stability experimental */
export type ExperimentalText = string
export declare function erasedAlias(x: ExperimentalText): ExperimentalText

// Type parameter constraints and defaults are directly represented.
export declare function constrained<T extends UnstableType>(x: T): T
export declare function defaulted<T = ExperimentalType>(): T
export interface GenericConstrained<T extends ExperimentalType> {
  value: T
}
export type IndexedAccess<K extends keyof UnstableType> = UnstableType[K]
export type ConditionalType<T> = T extends ExperimentalType ? ExperimentalType : string

// Traversal follows the declaration graph, not a fixed depth or member count.
interface Box<T> {
  value: T
}
export declare const deepGeneric: Box<Box<Box<Box<Box<ExperimentalType>>>>>
export interface OrderSensitive {
  deep: Box<Box<Box<Box<UnstableType>>>>
  direct: Box<ExperimentalType>
}

// Accessibility is declaration based: `__` is public, private identifiers and
// private constructors are not.
export interface PublicDoubleUnderscore {
  __public: ExperimentalType
}
export class PrivateIdentifierHidden {
  #secret: ExperimentalType | undefined
}
export class PrivateConstructor {
  private constructor(value: ExperimentalType) {}
}

// Callable surfaces expose their call signatures.
interface Callable {
  (value: ExperimentalType): void
}
export interface CallableOwner {
  value: Callable
}
export interface InlineCallable {
  value: (value: UnstableType) => void
}
