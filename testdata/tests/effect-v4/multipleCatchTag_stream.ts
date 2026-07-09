// @effect-diagnostics multipleCatchTag:warning
import { Data, Effect, Stream, pipe } from "effect"

class NotFound extends Data.TaggedError("NotFound")<{}> {}
class Unauthorized extends Data.TaggedError("Unauthorized")<{}> {}
class Retryable extends Data.TaggedError("Retryable")<{}> {}

declare const stream: Stream.Stream<string, NotFound | Unauthorized>

// Should trigger: consecutive catchTag transformations on Stream can collapse into catchTags
export const shouldReportStream = pipe(
  stream,
  Stream.catchTag("NotFound", () => Stream.succeed("missing")),
  Stream.catchTag("Unauthorized", () => Stream.succeed("denied"))
)

// Should not trigger: a non-catch transformation interrupts the chain
export const shouldNotReportInterrupted = pipe(
  stream,
  Stream.catchTag("NotFound", () => Stream.succeed("missing")),
  Stream.map((value) => value.toUpperCase()),
  Stream.catchTag("Unauthorized", () => Stream.succeed("denied"))
)

// Should not trigger: catchTag with orElse parameter must be ignored
export const shouldNotReportOrElse = pipe(
  stream,
  Stream.catchTag("NotFound", () => Stream.succeed("missing"), (error) => Stream.fail(error)),
  Stream.catchTag("Unauthorized", () => Stream.succeed("denied"))
)

// Should not trigger: a handled tag is rethrown as another tag that is later caught
export const shouldNotReportRethrow = pipe(
  stream,
  Stream.catchTag("NotFound", () => Stream.fail(new Retryable())),
  Stream.catchTag("Retryable", () => Stream.succeed("retried"))
)

void Effect
