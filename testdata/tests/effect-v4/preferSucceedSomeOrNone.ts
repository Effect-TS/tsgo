import { Effect, Effect as Fx, Option, Option as O, pipe } from "effect"
import { none as optionNone, some as optionSome } from "effect/Option"

// Should trigger: direct None constructor
export const shouldTriggerNone = Effect.succeed(Option.none())

// Should trigger: direct Some constructor
export const shouldTriggerSome = Effect.succeed(Option.some(1))

// Should trigger: aliased modules
export const shouldTriggerAliases = Fx.succeed(O.some("value"))

// Should trigger: named Option exports
export const shouldTriggerNamedNone = Effect.succeed(optionNone())
export const shouldTriggerNamedSome = Effect.succeed(optionSome(1))

// Should trigger: parenthesized Option call
export const shouldTriggerParenthesized = Effect.succeed((Option.some(1)))

// Should trigger: preserve an explicit Some type argument in the fix
export const shouldTriggerSomeTypeArgument = Effect.succeed(Option.some<number>(1))

// Should trigger: method-pipe forms
export const shouldTriggerNonePipe = Option.none().pipe(Effect.succeed)
export const shouldTriggerSomePipe = Option.some(1).pipe(Effect.succeed)

// Should trigger: Function.pipe forms
export const shouldTriggerNoneFunctionPipe = pipe(Option.none(), Effect.succeed)
export const shouldTriggerSomeFunctionPipe = pipe(Option.some(1), Effect.succeed)

// Should trigger: adjacent transformations in an existing flow
export const shouldTriggerTransformationPair = pipe(1, Option.some, Effect.succeed)

// Should trigger: replacing a Function.pipe prefix retains later transformations
export const shouldTriggerFunctionPipePrefix = pipe(1, Option.some, Effect.succeed, Effect.asVoid)

// Should trigger: replacing pipe prefixes retains later transformations
export const shouldTriggerNoneFunctionPipePrefix = pipe(Option.none(), Effect.succeed, Effect.asVoid)
export const shouldTriggerSomeMethodPipePrefix = Option.some(1).pipe(Effect.succeed, Effect.asVoid)
export const shouldTriggerNoneMethodPipePrefix = Option.none().pipe(Effect.succeed, Effect.asVoid)

// Should trigger: nested pipe styles can be mixed around the replaced prefix
export const shouldTriggerFunctionThenMethodPipe = pipe(Option.some(1), Effect.succeed).pipe(Effect.asVoid)
export const shouldTriggerMethodThenFunctionPipe = pipe(Option.some(1).pipe(Effect.succeed), Effect.asVoid)
export const shouldTriggerNestedPrefixAcrossPipeStyles = pipe(1, Option.some).pipe(Effect.succeed, Effect.asVoid)

// Should NOT trigger: an explicit None type argument cannot be preserved by Effect.succeedNone
export const shouldNotTriggerNoneTypeArgument = Effect.succeed(Option.none<number>())

// Should NOT trigger: an explicit outer type argument can intentionally widen the success type
export const shouldNotTriggerOuterTypeArgument = Effect.succeed<Option.Option<number>>(Option.none())

// Should NOT trigger: already using the direct constructors
export const shouldNotTriggerSucceedNone = Effect.succeedNone
export const shouldNotTriggerSucceedSome = Effect.succeedSome(1)

// Should NOT trigger: the Option functions are values rather than calls
export const shouldNotTriggerNoneFunction = Effect.succeed(Option.none)
export const shouldNotTriggerSomeFunction = Effect.succeed(Option.some)

const LocalEffect = {
  succeed: <A>(value: A) => Effect.succeed(value)
}
const LocalOption = {
  none: () => Option.none(),
  some: <A>(value: A) => Option.some(value)
}

// Should NOT trigger: look-alike APIs do not resolve to Effect's exports
export const shouldNotTriggerLocalEffect = LocalEffect.succeed(Option.none())
export const shouldNotTriggerLocalOptionNone = Effect.succeed(LocalOption.none())
export const shouldNotTriggerLocalOptionSome = Effect.succeed(LocalOption.some(1))
