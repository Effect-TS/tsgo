// @effect-diagnostics missingPipeableSignature:warning

export const missing = (self: ReadonlyArray<string>, index: number): string => self[index]

export const missingSubjectLast = (index: number, self: ReadonlyArray<string>): string => self[index]

export const valid: {
  (index: number): (self: ReadonlyArray<string>) => string
  (self: ReadonlyArray<string>, index: number): string
} = null as any

type Equivalence<A> = (self: A, that: A) => boolean

// A callable result must not make the data-first signature look pipeable.
export const callableResult: {
  <A>(that: Equivalence<A>): (self: Equivalence<A>) => Equivalence<A>
  <A>(self: Equivalence<A>, that: Equivalence<A>): Equivalence<A>
} = null as any

// The piped subject may be the final parameter.
export const subjectLast: {
  (prefix: string, index: number): (self: ReadonlyArray<string>) => string
  (prefix: string, index: number, self: ReadonlyArray<string>): string
} = null as any

// Pipeable overloads can take multiple outer arguments.
export const multipleOuter: {
  (prefix: string, index: number): (self: ReadonlyArray<string>) => string
  (self: ReadonlyArray<string>, prefix: string, index: number): string
} = null as any

// Rest signatures and signatures with fewer than two parameters are outside the rule.
export const rest = (...values: Array<string>): string => values.join("")
export const unary = (value: string): string => value
export const value = 1

const aliasedMissing = (self: ReadonlyArray<string>, index: number): string => self[index]
export { aliasedMissing }
