// @effect-diagnostics *:off
// @effect-diagnostics matchBooleanToIsSuccess:suggestion
import * as E from "effect"
import { Effect } from "effect"

declare const source: Effect.Effect<number, string>
declare const saveUser: Effect.Effect<void, Error>
const handlers = { onFailure: () => false, onSuccess: () => true }
declare const overrides: Partial<typeof handlers>

// MUST TRIGGER: return type annotations on the handlers -> Effect.isSuccess
export const annotatedReturn = Effect.match(source, { onFailure: (): boolean => false, onSuccess: (): boolean => true })

// MUST TRIGGER: annotated, unused parameters -> Effect.isSuccess
export const annotatedParameter = Effect.match(source, { onFailure: (_: string) => false, onSuccess: (_: number) => true })

// MUST TRIGGER: explicit type arguments on the pipeable form, dropped with the options -> Effect.isFailure
export const typeArguments = source.pipe(Effect.match<string, boolean, number, boolean>({ onFailure: () => true, onSuccess: () => false }))

// MUST TRIGGER: explicit type arguments on the data-first form -> Effect.isSuccess
export const dataFirstTypeArguments = Effect.match<number, string, never, boolean, boolean>(source, { onFailure: () => false, onSuccess: () => true })

// MUST TRIGGER: nested namespace qualifier is kept -> E.Effect.isSuccess
export const nestedQualifier = E.Effect.match(saveUser, { onFailure: () => false, onSuccess: () => true })

// MUST TRIGGER: data-first match nested as a later data-first argument -> Effect.isSuccess
export const nestedDataFirst = Effect.andThen(Effect.void, Effect.match(saveUser, { onFailure: () => false, onSuccess: () => true }))

// MUST TRIGGER: data-first match as the subject of a data-first call -> Effect.isFailure
export const dataFirstSubject = Effect.map(Effect.match(saveUser, { onFailure: () => true, onSuccess: () => false }), (failed) => !failed)

// MUST TRIGGER: pipeable match nested inside a pipe step's argument -> Effect.isSuccess
export const nestedPipeable = Effect.andThen(Effect.void, saveUser.pipe(Effect.match({ onFailure: () => false, onSuccess: () => true })))

// MUST TRIGGER: point-free match applied immediately and piped on -> Effect.isSuccess
export const appliedThenPiped = Effect.match({ onFailure: () => false, onSuccess: () => true })(saveUser).pipe(Effect.map((ok) => !ok))

// MUST NOT TRIGGER: an unapplied match passed as a callback is not a piping step
export const pointFreeArgument = Effect.flatMap(Effect.succeed(saveUser), Effect.match({ onFailure: () => true, onSuccess: () => false }))

// MUST TRIGGER: extra properties do not change the handlers -> Effect.isSuccess
export const extraProperty = Effect.match(source, { onFailure: () => false, onSuccess: () => true, extra: () => true } as never)

// MUST TRIGGER: a spread before the handlers is overridden by them -> Effect.isSuccess
export const spreadAndInline = Effect.match(source, { ...handlers, onFailure: () => false, onSuccess: () => true })

// MUST NOT TRIGGER: a spread after the handlers may override them
export const spreadAfterInline = Effect.match(source, { onFailure: () => false, onSuccess: () => true, ...overrides })

// MUST NOT TRIGGER: async handlers return promises
export const asyncHandlers = Effect.match(source, { onFailure: async () => false, onSuccess: async () => true })

// MUST NOT TRIGGER: rest parameter
export const restParameter = Effect.match(source, { onFailure: (..._args: Array<string>) => false, onSuccess: () => true })

// MUST NOT TRIGGER: more than one parameter
export const twoParameters = Effect.match(source, { onFailure: (_a: string, _b?: number) => false, onSuccess: () => true })

// MUST NOT TRIGGER: generic arrow
export const genericArrow = Effect.match(source, { onFailure: <T>() => false, onSuccess: () => true })

// MUST NOT TRIGGER: negated literal
export const negatedLiteral = Effect.match(source, { onFailure: () => !true, onSuccess: () => true })

// MUST NOT TRIGGER: satisfies around the literal
export const satisfiesLiteral = Effect.match(source, { onFailure: () => false satisfies boolean, onSuccess: () => true })

// MUST NOT TRIGGER: handler taken from another object
export const shorthand = Effect.match(source, { onFailure: handlers.onFailure, onSuccess: () => true })

// MUST NOT TRIGGER: a block body with statements before the return
export const multiStatementBlock = Effect.match(source, { onFailure: () => { console.log("failed"); return false }, onSuccess: () => true })
