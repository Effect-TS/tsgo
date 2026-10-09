// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning

/** @stability experimental */
interface InferredExperimental {
  value: string
}
declare const inferredValue: InferredExperimental

// Exported callable values keep their public signatures, whether annotated or
// inferred.
export const arrowExplicit = (x: InferredExperimental): InferredExperimental => x
export const arrowInferred = (x: InferredExperimental) => x
export const functionExpression = function (x: InferredExperimental): InferredExperimental {
  return x
}

interface InferredCallable {
  (x: InferredExperimental): InferredExperimental
}
declare const inferredCallable: InferredCallable
export const callableReference = inferredCallable

// Directly represented inferred object surfaces.
export const inferredObject = { value: inferredValue }
export const nestedObject = { nested: { value: inferredValue } }
export function inferredReturnObject() {
  return { value: inferredValue }
}

// Class surfaces: inferred properties, accessors and expressions.
export class InferredClass {
  property = { value: inferredValue }
  get accessor() {
    return inferredValue
  }
  arrow = (x: InferredExperimental): InferredExperimental => x
}
export const classExpression = class {
  value: InferredExperimental = inferredValue
}

// Inferred parameters, including constructors.
export function inferredParameter(x = inferredValue): void {}
export class InferredFactory {
  constructor(x = inferredValue) {}
}

// Type queries over generic and inferred values.
declare function genericFunction<T>(x: InferredExperimental): T
export type GenericFunctionType = typeof genericFunction
const inferredArrowValue = (x: InferredExperimental): InferredExperimental => x
export type InferredArrowType = typeof inferredArrowValue

// Generic callable surfaces are read from their raw signatures without
// instantiating them.
interface GenericCallable<T> {
  (x: InferredExperimental): T
}
export declare const genericCallable: GenericCallable<string>
