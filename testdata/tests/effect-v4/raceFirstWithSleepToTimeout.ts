// @effect-v4
// @effect-diagnostics *:off
// @effect-diagnostics raceFirstWithSleepToTimeout:warning
import { Effect, pipe } from "effect"

declare const work: Effect.Effect<string, Error>
declare const other: Effect.Effect<number, TypeError>

// Should report for a data-first sleep-then-fail left arm.
export const dataFirstLeft = Effect.raceFirst(
  Effect.sleep("1 second").pipe(
    Effect.flatMap(() => Effect.fail("timeout"))
  ),
  work
)

// Should report for a data-first sleep-then-fallback right arm.
export const dataFirstRight = Effect.raceFirst(
  work,
  Effect.flatMap(Effect.sleep("2 seconds"), () => Effect.succeed("fallback"))
)

// Should report in pipeable and free-pipe forms.
export const pipeable = work.pipe(
  Effect.raceFirst(
    Effect.sleep("3 seconds").pipe(Effect.as("fallback"))
  )
)
export const freePipe = pipe(
  Effect.sleep("4 seconds").pipe(Effect.andThen(Effect.fail("timeout"))),
  Effect.raceFirst(work)
)

// Should report when the timer is expressed with delay.
export const delayedFallback = Effect.raceFirst(
  work,
  Effect.delay(Effect.succeed("fallback"), "5 seconds")
)

// Should report for a literal two-arm raceAllFirst.
export const raceAllFirst = Effect.raceAllFirst([
  work,
  Effect.sleep("6 seconds").pipe(Effect.map(() => "fallback"))
])

// Should not report when neither or both arms are timers.
export const neitherTimer = Effect.raceFirst(work, other)
export const bothTimers = Effect.raceFirst(
  Effect.sleep("1 second"),
  Effect.delay(Effect.succeed("fallback"), "2 seconds")
)

// Should not report first-success races, whose failure behavior differs.
export const race = Effect.race(
  work,
  Effect.sleep("1 second").pipe(Effect.andThen(Effect.fail("timeout")))
)
export const raceAll = Effect.raceAll([
  work,
  Effect.sleep("1 second").pipe(Effect.as("fallback"))
])

// Should not report when raceFirst carries behavior timeoutOrElse cannot preserve.
export const withOptions = Effect.raceFirst(work, Effect.sleep("1 second"), {
  onWinner: () => {}
})

// Should not report a real-work arm that merely contains sleep later.
export const containsSleep = Effect.raceFirst(
  work.pipe(Effect.flatMap(() => Effect.sleep("1 second"))),
  other
)

// Should not report a sleep-rooted arm with a non-pure trailing operation.
export const trailingTap = Effect.raceFirst(
  work,
  Effect.sleep("1 second").pipe(Effect.tap(() => Effect.log("awake")))
)

// Should not report non-literal or non-binary raceAllFirst inputs.
declare const arms: ReadonlyArray<Effect.Effect<string, Error>>
export const dynamicRaceAllFirst = Effect.raceAllFirst(arms)
export const threeWayRaceAllFirst = Effect.raceAllFirst([
  work,
  other,
  Effect.sleep("1 second")
])
