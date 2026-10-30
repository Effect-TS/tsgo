// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics mapConstToAs:warning
import { Effect } from "effect"

declare const source: Effect.Effect<number>
declare const ambient: string
declare let ambientLet: string
const enum Flag { On = 1 }

// MUST TRIGGER: string literal body
export const literal = source.pipe(Effect.map(() => "saved"))

// MUST TRIGGER: negative bigint literal
export const negativeBigint = source.pipe(Effect.map(() => -1n))

// MUST TRIGGER: const enum member
export const constEnum = source.pipe(Effect.map(() => Flag.On))

// MUST TRIGGER: ambient const binding
export const ambientConst = source.pipe(Effect.map(() => ambient))

// MUST NOT TRIGGER: ambient mutable binding
export const ambientMutable = source.pipe(Effect.map(() => ambientLet))

// MUST NOT TRIGGER: ambient const declared after the call
export const ambientLater = source.pipe(Effect.map(() => laterAmbient))

// MUST NOT TRIGGER: explicit type arguments on the map
export const typeArguments = source.pipe(Effect.map<number, string>(() => "saved"))

// MUST NOT TRIGGER: rest parameter
export const restParameter = source.pipe(Effect.map((..._args: Array<number>) => "saved"))

// MUST NOT TRIGGER: async callback
export const asyncCallback = source.pipe(Effect.map(async () => "saved"))

// MUST NOT TRIGGER: unary plus is not a negative literal
export const notNegative = source.pipe(Effect.map(() => +1))

// MUST NOT TRIGGER: function expression with a return type annotation
export const annotatedFunction = source.pipe(Effect.map(function(): number { return 1 }))

declare const laterAmbient: string
