// @effect-diagnostics *:off
// @effect-diagnostics mapMapErrorToMapBoth:warning
import { Effect, pipe } from "effect"

declare const source: Effect.Effect<number, string>
declare const makeSource: () => Effect.Effect<number, string>
declare const makeSuccess: () => (n: number) => number
declare const makeFailure: () => (message: string) => number

// MUST TRIGGER (no quickfix): both steps curried; JavaScript evaluates makeFailure() before
// makeSuccess(), while Effect.mapBoth({ onSuccess, onFailure }) would evaluate them in flow order
export const curriedFactories = Effect.mapError(makeFailure())(Effect.map(makeSuccess())(source))

// MUST TRIGGER: both steps curried with function literals; evaluating them in another order
// runs no code, so the quickfix is offered
export const curriedLiterals = Effect.mapError((message: string) => message.length)(
  Effect.map((n: number) => n + 1)(source)
)

// MUST TRIGGER (no quickfix): curried first step, pipeable second step; makeSuccess() is
// evaluated before makeSource(), the merged step would evaluate makeSource() first
export const curriedThenPipeFactory = Effect.map(makeSuccess())(makeSource()).pipe(
  Effect.mapError((message) => message.length)
)

// MUST TRIGGER: same shape with a function literal first callback; the quickfix is offered
export const curriedThenPipeLiteral = Effect.map((n: number) => n + 1)(makeSource()).pipe(
  Effect.mapError(makeFailure())
)

// MUST TRIGGER (no quickfix): pipeable first step, curried second step; makeSource() is
// evaluated before makeSuccess(), the merged curried step would evaluate makeSuccess() first
export const pipeThenCurriedFactory = Effect.mapError((message: string) => message.length)(
  makeSource().pipe(Effect.map(makeSuccess()))
)

// MUST TRIGGER (no quickfix): explicit type arguments on the curried application
export const curriedApplicationTypeArguments = Effect.map((n: number) => n + 1)<number, never>(
  Effect.mapError((message: string) => message.length)(source)
)

// MUST TRIGGER (no quickfix): explicit type arguments on the curried factory
export const curriedFactoryTypeArguments = Effect.map<number, number>((n) => n + 1)(
  Effect.mapError((message: string) => message.length)(source)
)

// MUST TRIGGER (no quickfix): explicit type arguments on the pipe call that holds both steps;
// removing a step would leave a type argument without its step
export const pipeTypeArguments = pipe<
  Effect.Effect<number, string>,
  Effect.Effect<number, string>,
  Effect.Effect<number, number>
>(source, Effect.map((n) => n + 1), Effect.mapError((message) => message.length))

// MUST TRIGGER (no quickfix): eager curried steps with factories, same reordering
export const eagerCurriedFactories = Effect.mapErrorEager(makeFailure())(Effect.mapEager(makeSuccess())(source))

// MUST TRIGGER: eager curried steps with function literals; the quickfix is offered
export const eagerCurriedLiterals = Effect.mapErrorEager((message: string) => message.length)(
  Effect.mapEager((n: number) => n + 1)(source)
)
