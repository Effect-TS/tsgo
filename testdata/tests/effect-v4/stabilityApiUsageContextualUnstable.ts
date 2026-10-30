// @filename: argument.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
x({ test: 42 })
export {}

// @filename: shorthand.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
const test = 42; x({ test })
export {}

// @filename: quotedKey.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
x({ "test": 42 })
export {}

// @filename: computedLiteralKey.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
x({ ["test"]: 42 })
export {}

// @filename: typedVariable.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
const value: Reportable = { test: 42 }
export {}

// @filename: assignment.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
let value: Reportable; value = { test: 42 }
export {}

// @filename: satisfies.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
const value = { test: 42 } satisfies Reportable
export {}

// @filename: return.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
function make(): Reportable { return { test: 42 } }
export {}

// @filename: nested.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
const value: { inner: Reportable } = { inner: { test: 42 } }
export {}

// @filename: arrayElement.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
const value: Reportable[] = [{ test: 42 }]
export {}

// @filename: genericConstraint.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
declare function generic<T extends Reportable>(arg: T): void; generic({ test: 42 })
export {}

// @filename: generatorContextualProperty.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
const gen = function* () { x({ test: 42 }) }
export {}

// @filename: inferredGeneratorConstraint.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
declare function generic<T extends Reportable>(arg: T): void; const gen = function* () { generic({ test: 42 }) }
export {}

// @filename: nestedFunctionConstraint.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
declare function generic<T extends Reportable>(arg: T): void; const gen = function* () { function inner() { generic({ test: 42 }) } }
export {}

// @filename: annotatedInnerGeneratorConstraint.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
declare function generic<T extends Reportable>(arg: T): void; const gen = function* () { const inner = function* (): Generator<never, void, unknown> { generic({ test: 42 }) } }
export {}

// @filename: annotatedGeneratorConstraint.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
declare function generic<T extends Reportable>(arg: T): void; const gen = function* (): Generator<never, void, unknown> { generic({ test: 42 }) }
export {}

// @filename: generatorDeclarationConstraint.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
declare function generic<T extends Reportable>(arg: T): void; function* gen() { generic({ test: 42 }) }
export {}

// @filename: generatorMethodConstraint.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
declare function generic<T extends Reportable>(arg: T): void; class C { *gen() { generic({ test: 42 }) } }
export {}

// @filename: ordinaryFunctionConstraint.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
declare function generic<T extends Reportable>(arg: T): void; function gen() { generic({ test: 42 }) }
export {}

// @filename: arrowConstraint.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
declare function generic<T extends Reportable>(arg: T): void; const gen = () => generic({ test: 42 })
export {}

// @filename: nestedConstraint.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
declare function generic<T extends { inner: Reportable }>(arg: T): void; generic({ inner: { test: 42 } })
export {}

// @filename: arrayConstraint.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
declare function generic<T extends Reportable[]>(arg: T): void; generic([{ test: 42 }])
export {}

// @filename: restConstraint.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
declare function generic<T extends Reportable>(...args: T[]): void; generic({ stable: 1 }, { test: 42 })
export {}

// @filename: constructorConstraint.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
declare class Box<T extends Reportable> { constructor(arg: T) } new Box({ test: 42 })
export {}

// @filename: constraintUsingInference.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
declare function generic<A, T extends Partial<A>>(a: A, arg: T): void; declare const reportable: Reportable; generic(reportable, { test: 42 })
export {}

// @filename: constraintWithoutProperty.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
declare function generic<T extends { other?: number }>(arg: T): void; generic({ test: 42 })
export {}

// @filename: unconstrainedAlternative.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
declare function generic<T, U extends Reportable>(arg: T | U): void; generic({ test: 42 })
export {}

// @filename: fixedTupleSpread.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
declare function generic<T extends Reportable>(prefix: string, arg: T): void; declare const prefix: [string]; generic(...prefix, { test: 42 })
export {}

// @filename: fixedTupleSpreads.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
declare function generic<T extends Reportable>(a: string, b: number, c: boolean, arg: T): void; declare const ab: readonly [string, number]; declare const c: [boolean]; generic(...ab, ...c, { test: 42 })
export {}

// @filename: returnTypeInference.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
interface Box<A> { readonly value: A } declare function succeed<A>(a: A): Box<A>; function make(): Box<Reportable> { return succeed({ test: 42 }) }
export {}

// @filename: constrainedReturnTypeInference.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
interface Box<A> { readonly value: A } declare function succeed<A extends object>(a: A): Box<A>; const value: Box<Reportable> = succeed({ test: 42 })
export {}

// @filename: callbackReturnInference.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
declare function map<A, B>(as: A[], f: (a: A) => B): B[]; const value: Reportable[] = map([1], () => ({ test: 42 }))
export {}

// @filename: defaultTypeArgument.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
interface Plain { readonly test?: number } declare function withDefault<T extends Plain = Reportable>(arg: T): void; withDefault({ test: 42 })
export {}

// @filename: taggedTemplateConstraint.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
declare function tag<T extends Reportable>(s: TemplateStringsArray, v: T): void; tag`${{ test: 42 }}`
export {}

// @filename: explicitOuterTypeArguments.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
declare function ref<T>(key: string, o: { make: () => T }): T; declare function constant<A>(a: A): () => A; ref<Reportable>("k", { make: constant({ test: 42 }) })
export {}

// @filename: nestedGenericCall.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
declare function id<X>(x: X): X; declare function generic<T extends Reportable>(arg: T): void; generic(id({ test: 42 }))
export {}

// @filename: genericOwnDeclaration.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
declare function generic<T>(arg: T): void; generic({
/** @stability unstable */
test: 42 })
export {}

// @filename: inherited.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
interface Child extends Reportable {} const value: Child = { test: 42 }
export {}

// @filename: propertyAccess.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
declare function make(): Reportable; make().test
export {}

// @filename: valueReference.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
/** @stability unstable */
const value = 42; x({ stable: value })
export {}

// @filename: stableProperty.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
x({ stable: 42 })
export {}

// @filename: omittedProperty.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
x({})
export {}

// @filename: untypedLiteral.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
const value = { test: 42 }; x(value)
export {}

// @filename: untypedShorthand.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
const test = 42; const value = { test }; x(value)
export {}

// @filename: spread.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
const value = { test: 42 }; x({ ...value })
export {}

// @filename: differentProperty.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
const value: { test?: number } = { test: 42 }
export {}

// @filename: stableUnionBranch.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
type Choice = (Reportable & { kind: "experimental" }) | { kind: "stable"; test?: number }; const value: Choice = { kind: "stable", test: 42 }
export {}

// @filename: experimentalUnionBranch.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
type Choice = (Reportable & { kind: "experimental" }) | { kind: "stable"; test?: number }; const value: Choice = { kind: "experimental", test: 42 }
export {}

// @filename: ambiguousUnion.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
type Choice = Reportable | { test?: string }; const value: Choice = { test: 42 }
export {}

// @filename: unionDeduplication.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
type Choice = (Reportable & { kind?: "a" }) | (Reportable & { kind?: "b" }); const value: Choice = { test: 42 }
export {}

// @filename: method.ts
// @effect-v4
// @effect-diagnostics experimentalApiUsage:warning unstableApiUsage:warning
/** @stability stable */
interface Reportable {
  /** @stability unstable */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
/** @stability stable */
interface Methods {
/** @stability unstable */
test(): number }; const value: Methods = { test() { return 42 } }
export {}
