// @effect-v3
import { beforeEach, it, test } from "@effect/vitest"
import { Effect } from "effect"

declare const arbitrary: any

it("direct", () => Effect.void)
it.only("only", () => Effect.void)
it.each([1, 2])("each %s", () => Effect.void)
it.prop("property", [arbitrary], () => Effect.void)
test("test alias", () => Effect.void)
beforeEach(() => Effect.void)

it.effect("effect aware", () => Effect.void)
it.live("live aware", () => Effect.void)
it.scoped("scoped aware", () => Effect.void)
it.scopedLive("scoped live aware", () => Effect.void)
it("runPromise", () => Effect.runPromise(Effect.void))
