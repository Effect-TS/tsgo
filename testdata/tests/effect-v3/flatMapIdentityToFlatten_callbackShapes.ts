// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics flatMapIdentityToFlatten:warning
import { Channel, Effect, identity } from "effect"

declare const source: Effect.Effect<Effect.Effect<string>>
declare const fallback: Effect.Effect<string>

// MUST TRIGGER: plain identity arrow
export const valid = source.pipe(Effect.flatMap((inner) => inner))

// MUST NOT TRIGGER: annotated parameter widening the error channel
export const widened = source.pipe(Effect.flatMap((inner: Effect.Effect<string, unknown>) => inner))

// MUST NOT TRIGGER: explicit type arguments on flatMap
export const typeArguments = source.pipe(Effect.flatMap<Effect.Effect<string>, string, never, never>(identity))

// MUST NOT TRIGGER: async callback
export const asyncCallback = source.pipe(Effect.flatMap(async (inner) => inner))

// MUST NOT TRIGGER: callback with a second parameter
export const twoParameters = source.pipe(Effect.flatMap((inner, _extra?: number) => inner))

// MUST NOT TRIGGER: rest parameter
export const restParameter = source.pipe(Effect.flatMap((...inner: [Effect.Effect<string>]) => inner))

// MUST NOT TRIGGER: parameter initializer
export const initializer = source.pipe(Effect.flatMap((inner = fallback) => inner))

// MUST NOT TRIGGER: optional parameter
export const optionalParameter = source.pipe(Effect.flatMap((inner?: Effect.Effect<string>) => inner))

// MUST NOT TRIGGER: destructured parameter
export const destructured = source.pipe(Effect.flatMap(({ pipe }) => pipe()))

// MUST NOT TRIGGER: Channel.identity is not the Function identity
export const channelIdentity = source.pipe(Effect.flatMap(Channel.identity))

// MUST NOT TRIGGER: data-last call with identity
export const dataLastIdentity = Effect.flatMap(identity)(source)

// MUST NOT TRIGGER: an annotated alias of identity with the exact type
const exactAlias: (x: Effect.Effect<string>) => Effect.Effect<string> = identity
export const typedAliasExact = source.pipe(Effect.flatMap(exactAlias))

// MUST NOT TRIGGER: async function expression
export const asyncFunctionExpression = source.pipe(Effect.flatMap(async function(inner) { return inner }))

// MUST NOT TRIGGER: generator function expression
export const generatorFunctionExpression = source.pipe(Effect.flatMap(function*(inner) { return inner }))

// MUST NOT TRIGGER: function expression with a widening parameter annotation
export const typedFunctionExpression = source.pipe(Effect.flatMap(function(inner: Effect.Effect<string, unknown>) { return inner }))
